/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use bytes::BytesMut;
use openssl::x509::X509VerifyResult;
use slog::slog_info;
use tokio::io::{AsyncRead, AsyncWrite};

use g3_dpi::Protocol;
use g3_io_ext::{AsyncStream, OnceBufReader};
use g3_slog_types::{LtUpstreamAddr, LtUuid, LtX509VerifyResult};
use g3_types::net::{TlsServiceType, UpstreamAddr};
use g3_udpdump::{ExportedPduDissectorHint, StreamDumpProxyAddresses};

#[cfg(not(feature = "vendored-tongsuo"))]
use super::tls::ParsedClientHello;
use super::{
    BoxAsyncRead, BoxAsyncWrite, InterceptionError, StreamInspectContext, StreamInspection,
    TlsInterceptionContext,
};
use crate::config::server::ServerConfig;
use crate::inspect::tls::TlsInterceptionError;
use crate::log::inspect::InspectSource;
use crate::log::inspect::stream::StreamInspectLog;
use crate::serve::ServerTaskResult;

#[cfg(feature = "vendored-tongsuo")]
mod tlcp;
mod tls;

macro_rules! intercept_log {
    ($obj:tt, $($args:tt)+) => {
        if let Some(logger) = $obj.ctx.intercept_logger() {
            slog_info!(logger, $($args)+;
                "intercept_type" => "StartTlsHandshake",
                "task_id" => LtUuid($obj.ctx.server_task_id()),
                "depth" => $obj.ctx.inspection_depth,
                "upstream" => LtUpstreamAddr(&$obj.upstream),
                "protocol" => Protocol::from($obj.protocol).as_str(),
                "tls_server_verify" => $obj.server_verify_result.map(LtX509VerifyResult),
            );
        }
    };
}

#[derive(Clone, Copy)]
pub(crate) enum StartTlsProtocol {
    Smtp,
    #[allow(unused)]
    Imap,
}

impl From<StartTlsProtocol> for Protocol {
    fn from(value: StartTlsProtocol) -> Self {
        match value {
            StartTlsProtocol::Smtp => Protocol::Smtp,
            StartTlsProtocol::Imap => Protocol::Imap,
        }
    }
}

impl From<StartTlsProtocol> for TlsServiceType {
    fn from(value: StartTlsProtocol) -> Self {
        match value {
            StartTlsProtocol::Smtp => TlsServiceType::Smtp,
            StartTlsProtocol::Imap => TlsServiceType::Imap,
        }
    }
}

struct StartTlsInterceptIo {
    pub(super) clt_r: BoxAsyncRead,
    pub(super) clt_w: BoxAsyncWrite,
    pub(super) ups_r: BoxAsyncRead,
    pub(super) ups_w: BoxAsyncWrite,
}

pub(crate) struct StartTlsInterceptObject<SC: ServerConfig> {
    io: Option<StartTlsInterceptIo>,
    ctx: StreamInspectContext<SC>,
    upstream: UpstreamAddr,
    tls_interception: TlsInterceptionContext,
    protocol: StartTlsProtocol,
    server_verify_result: Option<X509VerifyResult>,
}

impl<SC> StartTlsInterceptObject<SC>
where
    SC: ServerConfig + Send + Sync + 'static,
{
    pub(super) fn new(
        ctx: StreamInspectContext<SC>,
        upstream: UpstreamAddr,
        tls: TlsInterceptionContext,
        protocol: StartTlsProtocol,
    ) -> Self {
        StartTlsInterceptObject {
            io: None,
            ctx,
            upstream,
            tls_interception: tls,
            protocol,
            server_verify_result: None,
        }
    }

    pub(super) fn set_io(
        &mut self,
        clt_r: BoxAsyncRead,
        clt_w: BoxAsyncWrite,
        ups_r: BoxAsyncRead,
        ups_w: BoxAsyncWrite,
    ) {
        let io = StartTlsInterceptIo {
            clt_r,
            clt_w,
            ups_r,
            ups_w,
        };
        self.io = Some(io);
    }

    pub(super) async fn intercept(mut self) -> ServerTaskResult<StreamInspection<SC>> {
        match self.do_intercept().await {
            Ok(obj) => {
                intercept_log!(self, "ok");
                Ok(obj)
            }
            Err(e) => {
                intercept_log!(self, "{e}");
                Err(InterceptionError::StartTls(e).into_server_task_error(Protocol::TlsModern))
            }
        }
    }

    async fn do_intercept(&mut self) -> Result<StreamInspection<SC>, TlsInterceptionError> {
        let StartTlsInterceptIo {
            mut clt_r,
            clt_w,
            ups_r,
            ups_w,
        } = self.io.take().unwrap();

        let mut clt_r_buf = BytesMut::with_capacity(2048);
        let client_hello = self
            .tls_interception
            .read_client_hello(&mut clt_r, &mut clt_r_buf)
            .await?;

        self.set_io(clt_r, clt_w, ups_r, ups_w);
        if client_hello.version.is_tlcp() {
            self.do_intercept_tlcp(client_hello, clt_r_buf).await
        } else {
            self.do_intercept_tls(client_hello, clt_r_buf).await
        }
    }

    #[cfg(not(feature = "vendored-tongsuo"))]
    pub(super) async fn do_intercept_tlcp(
        &mut self,
        _client_hello: ParsedClientHello,
        clt_r_buf: BytesMut,
    ) -> Result<StreamInspection<SC>, TlsInterceptionError> {
        let StartTlsInterceptIo {
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

    fn transfer_connected<CS, US>(&self, clt_s: CS, ups_s: US) -> StreamInspection<SC>
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

        let protocol = Protocol::from(self.protocol);
        if let Some(stream_dumper) = self
            .tls_interception
            .get_stream_dumper(self.ctx.task_notes.worker_id)
        {
            let dissector_hint = if !protocol.wireshark_dissector().is_empty() {
                ExportedPduDissectorHint::Protocol(protocol)
            } else {
                ExportedPduDissectorHint::TcpPort(self.upstream.port())
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
                self.inspect_inner(protocol, clt_r, clt_w, ups_r, ups_w)
            } else {
                let (ups_r, ups_w) =
                    stream_dumper.wrap_proxy_remote_io(addresses, dissector_hint, ups_r, ups_w);
                self.inspect_inner(protocol, clt_r, clt_w, ups_r, ups_w)
            }
        } else {
            self.inspect_inner(protocol, clt_r, clt_w, ups_r, ups_w)
        }
    }

    fn inspect_inner<CR, CW, UR, UW>(
        &self,
        protocol: Protocol,
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
        StreamInspectLog::new(&ctx).log(InspectSource::StartTls, protocol);
        match self.protocol {
            StartTlsProtocol::Smtp => {
                let mut smtp_obj =
                    crate::inspect::smtp::SmtpInterceptObject::new(ctx, self.upstream.clone());
                smtp_obj.set_from_starttls();
                smtp_obj.set_io(
                    Box::new(clt_r),
                    Box::new(clt_w),
                    OnceBufReader::with_no_buf(Box::new(ups_r)),
                    Box::new(ups_w),
                );
                StreamInspection::Smtp(smtp_obj)
            }
            StartTlsProtocol::Imap => {
                let mut imap_obj =
                    crate::inspect::imap::ImapInterceptObject::new(ctx, self.upstream.clone());
                imap_obj.set_from_starttls();
                imap_obj.set_io(
                    Box::new(clt_r),
                    Box::new(clt_w),
                    OnceBufReader::with_no_buf(Box::new(ups_r)),
                    Box::new(ups_w),
                );
                StreamInspection::Imap(imap_obj)
            } /*
              _ => {
                  let mut stream_obj =
                      crate::inspect::stream::StreamInspectObject::new(ctx, self.upstream.clone());
                  stream_obj.set_io(
                      Box::new(clt_r),
                      Box::new(clt_w),
                      Box::new(ups_r),
                      Box::new(ups_w),
                  );
                  StreamInspection::StreamUnknown(stream_obj)
              }
               */
        }
    }
}
