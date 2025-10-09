/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::anyhow;
use bytes::BytesMut;
use openssl::x509::X509VerifyResult;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};
use tokio::runtime::Handle;

use g3_cert_agent::CertAgentHandle;
use g3_dpi::parser::tls::{
    ClientHello, ExtensionType, HandshakeCoalescer, RawVersion, Record, RecordParseError,
};
use g3_dpi::{Protocol, ProtocolInspector};
use g3_io_ext::{AsyncStream, FlexBufReader, OnceBufReader};
use g3_slog_types::{LtUpstreamAddr, LtUuid, LtX509VerifyResult};
use g3_types::net::{
    AlpnProtocol, OpensslInterceptionClientConfig, OpensslInterceptionServerConfig, TlsAlpn,
    TlsServerName, UpstreamAddr,
};
use g3_udpdump::{
    ExportedPduDissectorHint, StreamDumpConfig, StreamDumpProxyAddresses, StreamDumper,
};

use super::{
    BoxAsyncRead, BoxAsyncWrite, InterceptionError, StreamInspectContext, StreamInspection,
};
use crate::config::server::ServerConfig;
use crate::log::inspect::{InspectSource, stream::StreamInspectLog};
use crate::serve::ServerTaskResult;

mod error;
pub(crate) use error::TlsInterceptionError;

mod modern;
#[cfg(feature = "vendored-tongsuo")]
mod tlcp;

pub(super) struct ParsedClientHello {
    pub(super) version: RawVersion,
    pub(super) sni: Option<TlsServerName>,
    pub(super) alpn: Option<TlsAlpn>,
}

impl ParsedClientHello {
    pub(super) fn parse(ch: ClientHello<'_>) -> anyhow::Result<Self> {
        let mut sni: Option<TlsServerName> = None;
        let mut alpn: Option<TlsAlpn> = None;

        for ext in ch.ext_iter() {
            let ext = ext.map_err(|e| anyhow!("parse extension error: {e}"))?;
            let Some(data) = ext.data() else {
                continue;
            };

            match ext.r#type() {
                ExtensionType::ServerName => {
                    let v = TlsServerName::from_extension_value(data)
                        .map_err(|e| anyhow!("invalid server name extension: {e}"))?;
                    sni = Some(v);
                }
                ExtensionType::ApplicationLayerProtocolNegotiation => {
                    let v = TlsAlpn::from_extension_value(data)
                        .map_err(|e| anyhow!("invalid ALPN extension: {e}"))?;
                    alpn = Some(v);
                }
                _ => {}
            }
        }

        Ok(ParsedClientHello {
            version: ch.legacy_version,
            sni,
            alpn,
        })
    }
}

#[derive(Clone)]
pub(crate) struct TlsInterceptionContext {
    pub(super) cert_agent: Arc<CertAgentHandle>,
    pub(super) client_config: Arc<OpensslInterceptionClientConfig>,
    pub(super) server_config: Arc<OpensslInterceptionServerConfig>,
    stream_dumper: Arc<Vec<StreamDumper>>,
}

impl TlsInterceptionContext {
    pub(crate) fn new(
        cert_agent: CertAgentHandle,
        client_config: OpensslInterceptionClientConfig,
        server_config: OpensslInterceptionServerConfig,
        dump_config: Option<StreamDumpConfig>,
    ) -> anyhow::Result<Self> {
        let mut stream_dumper = Vec::new();
        if let Some(dump) = dump_config {
            g3_daemon::runtime::worker::foreach(|h| {
                let dumper = StreamDumper::new(dump, &h.handle).map_err(|e| {
                    anyhow!("failed to create tls stream dumper in worker {}: {e}", h.id)
                })?;
                stream_dumper.push(dumper);
                Ok::<(), anyhow::Error>(())
            })?;

            if stream_dumper.is_empty() {
                let dump_count =
                    g3_daemon::runtime::config::get_runtime_config().intended_thread_number();
                let handle = Handle::current();
                for i in 0..dump_count {
                    let dumper = StreamDumper::new(dump, &handle).map_err(|e| {
                        anyhow!("failed to create tls stream dumper #{i} in main runtime: {e}")
                    })?;
                    stream_dumper.push(dumper);
                }
            }
        }

        Ok(TlsInterceptionContext {
            cert_agent: Arc::new(cert_agent),
            client_config: Arc::new(client_config),
            server_config: Arc::new(server_config),
            stream_dumper: Arc::new(stream_dumper),
        })
    }

    pub(super) fn get_stream_dumper(&self, worker_id: Option<usize>) -> Option<&StreamDumper> {
        if self.stream_dumper.is_empty() {
            return None;
        }

        if let Some(id) = worker_id
            && let Some(d) = self.stream_dumper.get(id)
        {
            return Some(d);
        }

        fastrand::choice(self.stream_dumper.iter())
    }

    pub(super) async fn read_client_hello<R>(
        &mut self,
        clt_r: &mut R,
        clt_r_buf: &mut BytesMut,
    ) -> Result<ParsedClientHello, TlsInterceptionError>
    where
        R: AsyncRead + Unpin,
    {
        tokio::time::timeout(
            self.server_config.client_hello_recv_timeout,
            self.do_read_client_hello(clt_r, clt_r_buf),
        )
        .await
        .map_err(|_| TlsInterceptionError::ClientHandshakeTimeout)?
        .map_err(TlsInterceptionError::ClientHandshakeFailed)
    }

    async fn do_read_client_hello<R>(
        &mut self,
        clt_r: &mut R,
        clt_r_buf: &mut BytesMut,
    ) -> anyhow::Result<ParsedClientHello>
    where
        R: AsyncRead + Unpin,
    {
        let mut handshake_coalescer =
            HandshakeCoalescer::new(self.server_config.client_hello_max_size);
        let mut record_offset = 0;

        loop {
            let mut record = match Record::parse(&clt_r_buf[record_offset..]) {
                Ok(r) => r,
                Err(RecordParseError::NeedMoreData(_)) => match clt_r.read_buf(clt_r_buf).await {
                    Ok(0) => {
                        return Err(anyhow!("connection closed by client"));
                    }
                    Ok(_) => continue,
                    Err(e) => {
                        return Err(anyhow!("client read error: {e}"));
                    }
                },
                Err(_) => {
                    return Err(anyhow!("invalid tls client hello request"));
                }
            };
            record_offset += record.encoded_len();

            // The Client Hello Message MUST be the first Handshake message
            match record.consume_handshake(&mut handshake_coalescer) {
                Ok(Some(handshake_msg)) => {
                    let ch = handshake_msg
                        .parse_client_hello()
                        .map_err(|_| anyhow!("invalid tls client hello request"))?;
                    return ParsedClientHello::parse(ch);
                }
                Ok(None) => match handshake_coalescer.parse_client_hello() {
                    Ok(Some(ch)) => return ParsedClientHello::parse(ch),
                    Ok(None) => {
                        if !record.consume_done() {
                            return Err(anyhow!("partial fragmented tls client hello request"));
                        }
                    }
                    Err(_) => {
                        return Err(anyhow!("invalid fragmented tls client hello request"));
                    }
                },
                Err(_) => {
                    return Err(anyhow!("invalid tls client hello request"));
                }
            }
        }
    }
}

struct TlsInterceptIo {
    pub(super) clt_r_buf: BytesMut,
    pub(super) clt_r: BoxAsyncRead,
    pub(super) clt_w: BoxAsyncWrite,
    pub(super) ups_r: BoxAsyncRead,
    pub(super) ups_w: BoxAsyncWrite,
}

pub(crate) struct TlsInterceptObject<SC: ServerConfig> {
    io: Option<TlsInterceptIo>,
    ctx: StreamInspectContext<SC>,
    upstream: UpstreamAddr,
    tls_interception: TlsInterceptionContext,
    server_verify_result: Option<X509VerifyResult>,
}

macro_rules! intercept_log {
    ($obj:tt, $($args:tt)+) => {
        if let Some(logger) = $obj.ctx.intercept_logger() {
            slog::info!(logger, $($args)+;
                "intercept_type" => "TlsHandshake",
                "task_id" => LtUuid($obj.ctx.server_task_id()),
                "depth" => $obj.ctx.inspection_depth,
                "upstream" => LtUpstreamAddr(&$obj.upstream),
                "tls_server_verify" => $obj.server_verify_result.map(LtX509VerifyResult),
            );
        }
    };
}

impl<SC: ServerConfig> TlsInterceptObject<SC> {
    pub(crate) fn new(
        ctx: StreamInspectContext<SC>,
        upstream: UpstreamAddr,
        tls: TlsInterceptionContext,
    ) -> Self {
        TlsInterceptObject {
            io: None,
            ctx,
            upstream,
            tls_interception: tls,
            server_verify_result: None,
        }
    }

    pub(crate) fn set_io(
        &mut self,
        clt_r_buf: BytesMut,
        clt_r: BoxAsyncRead,
        clt_w: BoxAsyncWrite,
        ups_r: BoxAsyncRead,
        ups_w: BoxAsyncWrite,
    ) {
        let io = TlsInterceptIo {
            clt_r_buf,
            clt_r,
            clt_w,
            ups_r,
            ups_w,
        };
        self.io = Some(io);
    }

    fn log_ok(&self) {
        intercept_log!(self, "ok");
    }

    fn log_err(&self, e: &TlsInterceptionError) {
        intercept_log!(self, "{e}");
    }

    fn retain_alpn_protocol(&self, p: &[u8]) -> bool {
        if p == AlpnProtocol::Http2.identification_sequence() {
            return !self.ctx.h2_inspect_action(self.upstream.host()).is_block();
        } else if p == AlpnProtocol::Smtp.identification_sequence() {
            return !self
                .ctx
                .smtp_inspect_action(self.upstream.host())
                .is_block();
        } else if p == AlpnProtocol::Imap.identification_sequence() {
            return !self
                .ctx
                .imap_inspect_action(self.upstream.host())
                .is_block();
        }
        true
    }
}

impl<SC> TlsInterceptObject<SC>
where
    SC: ServerConfig + Send + Sync + 'static,
{
    pub(crate) async fn intercept(
        mut self,
        inspector: &mut ProtocolInspector,
    ) -> ServerTaskResult<StreamInspection<SC>> {
        match self.do_intercept(inspector).await {
            Ok(obj) => {
                self.log_ok();
                Ok(obj)
            }
            Err(e) => {
                self.log_err(&e);
                Err(InterceptionError::Tls(e).into_server_task_error(Protocol::TlsModern))
            }
        }
    }

    async fn do_intercept(
        &mut self,
        inspector: &mut ProtocolInspector,
    ) -> Result<StreamInspection<SC>, TlsInterceptionError> {
        let TlsInterceptIo {
            mut clt_r_buf,
            mut clt_r,
            clt_w,
            ups_r,
            ups_w,
        } = self.io.take().unwrap();

        let client_hello = self
            .tls_interception
            .read_client_hello(&mut clt_r, &mut clt_r_buf)
            .await?;

        self.set_io(clt_r_buf, clt_r, clt_w, ups_r, ups_w);

        if client_hello.version.is_tlcp() {
            self.do_intercept_tlcp(client_hello, inspector).await
        } else {
            self.do_intercept_modern(client_hello, inspector).await
        }
    }

    #[cfg(not(feature = "vendored-tongsuo"))]
    async fn do_intercept_tlcp(
        &mut self,
        _client_hello: ParsedClientHello,
        _inspector: &mut ProtocolInspector,
    ) -> Result<StreamInspection<SC>, TlsInterceptionError> {
        let TlsInterceptIo {
            clt_r_buf,
            clt_r,
            clt_w,
            ups_r,
            ups_w,
        } = self.io.take().unwrap();

        let mut stream_obj = crate::inspect::stream::StreamInspectObject::new(
            self.ctx.clone(),
            self.upstream.clone(),
        );
        stream_obj.set_io(
            Box::new(OnceBufReader::new(clt_r, clt_r_buf)),
            Box::new(clt_w),
            Box::new(ups_r),
            Box::new(ups_w),
        );
        // TLCP is not supported in this build mode, treat it as unknown protocol.
        // Unknown protocol should be forbidden if needed.
        Ok(StreamInspection::StreamUnknown(stream_obj))
    }

    fn transfer_connected<CS, US>(
        &self,
        protocol: Protocol,
        has_alpn: bool,
        clt_s: CS,
        ups_s: US,
    ) -> StreamInspection<SC>
    where
        CS: AsyncStream,
        CS::R: AsyncRead + Send + Sync + Unpin + 'static,
        CS::W: AsyncWrite + Send + Sync + Unpin + 'static,
        US: AsyncStream,
        US::R: AsyncRead + Send + Sync + Unpin + 'static,
        US::W: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        let (clt_r, clt_w) = clt_s.into_split();
        let (ups_r, ups_w) = ups_s.into_split();

        if let Some(stream_dumper) = self
            .tls_interception
            .get_stream_dumper(self.ctx.task_notes.worker_id)
        {
            let dissector_hint = if !protocol.wireshark_dissector().is_empty() {
                ExportedPduDissectorHint::Protocol(protocol)
            } else {
                ExportedPduDissectorHint::TlsPort(self.upstream.port())
            };
            let addresses = StreamDumpProxyAddresses {
                client: self.ctx.task_notes.client_addr,
                local_server: self.ctx.task_notes.server_addr,
                local_client: self.ctx.connect_notes.client_addr,
                remote: self.ctx.connect_notes.server_addr,
            };
            if stream_dumper.client_side() {
                let (clt_r, clt_w) =
                    stream_dumper.wrap_proxy_client_io(addresses, dissector_hint, clt_r, clt_w);
                self.inspect_inner(protocol, has_alpn, clt_r, clt_w, ups_r, ups_w)
            } else {
                let (ups_r, ups_w) =
                    stream_dumper.wrap_proxy_remote_io(addresses, dissector_hint, ups_r, ups_w);
                self.inspect_inner(protocol, has_alpn, clt_r, clt_w, ups_r, ups_w)
            }
        } else {
            self.inspect_inner(protocol, has_alpn, clt_r, clt_w, ups_r, ups_w)
        }
    }

    fn inspect_inner<CR, CW, UR, UW>(
        &self,
        protocol: Protocol,
        has_alpn: bool,
        clt_r: CR,
        clt_w: CW,
        ups_r: UR,
        ups_w: UW,
    ) -> StreamInspection<SC>
    where
        CR: AsyncRead + Send + Sync + Unpin + 'static,
        CW: AsyncWrite + Send + Sync + Unpin + 'static,
        UR: AsyncRead + Send + Sync + Unpin + 'static,
        UW: AsyncWrite + Send + Sync + Unpin + 'static,
    {
        let mut ctx = self.ctx.clone();
        ctx.increase_inspection_depth();
        StreamInspectLog::new(&ctx).log(InspectSource::TlsAlpn, protocol);
        match protocol {
            Protocol::Http1 => {
                let mut h1_obj = crate::inspect::http::H1InterceptObject::new(ctx);
                h1_obj.set_io(
                    FlexBufReader::new(Box::new(clt_r)),
                    Box::new(clt_w),
                    Box::new(ups_r),
                    Box::new(ups_w),
                );
                StreamInspection::H1(h1_obj)
            }
            Protocol::Http2 => {
                let mut h2_obj =
                    crate::inspect::http::H2InterceptObject::new(ctx, self.upstream.clone());
                h2_obj.set_io(
                    OnceBufReader::with_no_buf(Box::new(clt_r)),
                    Box::new(clt_w),
                    Box::new(ups_r),
                    Box::new(ups_w),
                );
                StreamInspection::H2(h2_obj)
            }
            Protocol::Smtp => {
                let mut smtp_obj =
                    crate::inspect::smtp::SmtpInterceptObject::new(ctx, self.upstream.clone());
                smtp_obj.set_io(
                    Box::new(clt_r),
                    Box::new(clt_w),
                    OnceBufReader::with_no_buf(Box::new(ups_r)),
                    Box::new(ups_w),
                );
                StreamInspection::Smtp(smtp_obj)
            }
            Protocol::Imap => {
                let mut imap_obj =
                    crate::inspect::imap::ImapInterceptObject::new(ctx, self.upstream.clone());
                imap_obj.set_io(
                    Box::new(clt_r),
                    Box::new(clt_w),
                    OnceBufReader::with_no_buf(Box::new(ups_r)),
                    Box::new(ups_w),
                );
                StreamInspection::Imap(imap_obj)
            }
            _ => {
                let mut stream_obj =
                    crate::inspect::stream::StreamInspectObject::new(ctx, self.upstream.clone());
                stream_obj.set_io(
                    Box::new(clt_r),
                    Box::new(clt_w),
                    Box::new(ups_r),
                    Box::new(ups_w),
                );
                if has_alpn {
                    // Just treat it as unknown. Unknown protocol should be forbidden if needed.
                    StreamInspection::StreamUnknown(stream_obj)
                } else {
                    // Inspect if no ALPN is set
                    StreamInspection::StreamInspect(stream_obj)
                }
            }
        }
    }
}
