/*
 * Copyright 2023 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::sync::Arc;

use anyhow::anyhow;
use openssl::x509::X509VerifyResult;
use slog::slog_info;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::runtime::Handle;

use g3_cert_agent::CertAgentHandle;
use g3_dpi::Protocol;
use g3_io_ext::{AsyncStream, FlexBufReader, OnceBufReader};
use g3_slog_types::{LtUpstreamAddr, LtUuid, LtX509VerifyResult};
use g3_types::net::{
    AlpnProtocol, OpensslInterceptionClientConfig, OpensslInterceptionServerConfig, UpstreamAddr,
};
use g3_udpdump::{
    ExportedPduDissectorHint, StreamDumpConfig, StreamDumpProxyAddresses, StreamDumper,
};

use super::{BoxAsyncRead, BoxAsyncWrite, StreamInspectContext, StreamInspection};
use crate::config::server::ServerConfig;
use crate::log::inspect::{InspectSource, stream::StreamInspectLog};

mod error;
pub(crate) use error::TlsInterceptionError;

mod modern;
#[cfg(feature = "vendored-tongsuo")]
mod tlcp;

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

        if let Some(id) = worker_id {
            if let Some(d) = self.stream_dumper.get(id) {
                return Some(d);
            }
        }

        fastrand::choice(self.stream_dumper.iter())
    }
}

struct TlsInterceptIo {
    pub(super) clt_r: OnceBufReader<BoxAsyncRead>,
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
            slog_info!(logger, $($args)+;
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
        clt_r: OnceBufReader<BoxAsyncRead>,
        clt_w: BoxAsyncWrite,
        ups_r: BoxAsyncRead,
        ups_w: BoxAsyncWrite,
    ) {
        let io = TlsInterceptIo {
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
