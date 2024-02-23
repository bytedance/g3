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
use slog::slog_info;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::runtime::Handle;

use g3_dpi::Protocol;
use g3_io_ext::{FlexBufReader, OnceBufReader};
use g3_slog_types::{LtUpstreamAddr, LtUuid};
use g3_tls_cert::agent::CertAgentHandle;
use g3_types::net::{
    OpensslInterceptionClientConfig, OpensslInterceptionServerConfig, UpstreamAddr,
};
use g3_udpdump::{ExportedPduDissectorHint, StreamDumpConfig, StreamDumper};

use super::{BoxAsyncRead, BoxAsyncWrite, StreamInspectContext, StreamInspection};
use crate::config::server::ServerConfig;
use crate::log::inspect::{stream::StreamInspectLog, InspectSource};

mod error;

pub(crate) use error::TlsInterceptionError;

mod modern;

#[derive(Clone)]
pub(crate) struct TlsInterceptionContext {
    cert_agent: Arc<CertAgentHandle>,
    client_config: Arc<OpensslInterceptionClientConfig>,
    server_config: Arc<OpensslInterceptionServerConfig>,
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

    fn get_stream_dumper(&self, worker_id: Option<usize>) -> Option<&StreamDumper> {
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
}

macro_rules! intercept_log {
    ($obj:tt, $($args:tt)+) => {
        slog_info!($obj.ctx.intercept_logger(), $($args)+;
            "intercept_type" => "TlsHandshake",
            "task_id" => LtUuid($obj.ctx.server_task_id()),
            "depth" => $obj.ctx.inspection_depth,
            "upstream" => LtUpstreamAddr(&$obj.upstream),
        )
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
        CS: AsyncRead + AsyncWrite + Send + Unpin + 'static,
        US: AsyncRead + AsyncWrite + Send + Unpin + 'static,
    {
        let (clt_r, clt_w) = tokio::io::split(clt_s);
        let (ups_r, ups_w) = tokio::io::split(ups_s);

        if let Some(stream_dumper) = self
            .tls_interception
            .get_stream_dumper(self.ctx.task_notes.worker_id)
        {
            let dissector_hint = if !protocol.wireshark_dissector().is_empty() {
                ExportedPduDissectorHint::Protocol(protocol)
            } else {
                ExportedPduDissectorHint::TlsPort(self.upstream.port())
            };
            let (ups_r, ups_w) = stream_dumper.wrap_remote_io(
                self.ctx.task_notes.client_addr,
                self.ctx.task_notes.server_addr,
                dissector_hint,
                ups_r,
                ups_w,
            );
            self.inspect_inner(protocol, has_alpn, clt_r, clt_w, ups_r, ups_w)
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
        CR: AsyncRead + Send + Unpin + 'static,
        CW: AsyncWrite + Send + Unpin + 'static,
        UR: AsyncRead + Send + Unpin + 'static,
        UW: AsyncWrite + Send + Unpin + 'static,
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
                let mut h2_obj = crate::inspect::http::H2InterceptObject::new(ctx);
                h2_obj.set_io(
                    OnceBufReader::with_no_buf(Box::new(clt_r)),
                    Box::new(clt_w),
                    Box::new(ups_r),
                    Box::new(ups_w),
                );
                StreamInspection::H2(h2_obj)
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
