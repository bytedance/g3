/*
 * Copyright 2024 ByteDance and/or its affiliates.
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
use openssl::ssl::Ssl;
use openssl::x509::X509VerifyResult;
use slog::slog_info;
use tokio::io::{AsyncRead, AsyncWrite};

use g3_dpi::Protocol;
use g3_io_ext::{AsyncStream, OnceBufReader};
use g3_openssl::{SslConnector, SslLazyAcceptor};
use g3_slog_types::{LtUpstreamAddr, LtUuid, LtX509VerifyResult};
use g3_types::net::{Host, TlsCertUsage, TlsServiceType, UpstreamAddr};
use g3_udpdump::ExportedPduDissectorHint;

use super::{
    BoxAsyncRead, BoxAsyncWrite, InterceptionError, StreamInspectContext, StreamInspection,
    TlsInterceptionContext,
};
use crate::config::server::ServerConfig;
use crate::inspect::tls::TlsInterceptionError;
use crate::log::inspect::stream::StreamInspectLog;
use crate::log::inspect::InspectSource;
use crate::serve::ServerTaskResult;

#[cfg(not(feature = "vendored-tongsuo"))]
const CERT_USAGE: TlsCertUsage = TlsCertUsage::TlsServer;
#[cfg(feature = "vendored-tongsuo")]
const CERT_USAGE: TlsCertUsage = TlsCertUsage::TLsServerTongsuo;

macro_rules! intercept_log {
    ($obj:tt, $($args:tt)+) => {
        slog_info!($obj.ctx.intercept_logger(), $($args)+;
            "intercept_type" => "StartTlsHandshake",
            "task_id" => LtUuid($obj.ctx.server_task_id()),
            "depth" => $obj.ctx.inspection_depth,
            "upstream" => LtUpstreamAddr(&$obj.upstream),
            "protocol" => Protocol::from($obj.protocol).as_str(),
            "tls_server_verify" => $obj.server_verify_result.map(LtX509VerifyResult),
        )
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

    pub(crate) fn set_io(
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

    pub(crate) async fn intercept(mut self) -> ServerTaskResult<StreamInspection<SC>> {
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
            clt_r,
            clt_w,
            ups_r,
            ups_w,
        } = self.io.take().unwrap();

        let ssl = Ssl::new(&self.tls_interception.server_config.ssl_context).map_err(|e| {
            TlsInterceptionError::InternalOpensslServerError(anyhow!(
                "failed to get new SSL state: {e}"
            ))
        })?;
        let mut lazy_acceptor =
            SslLazyAcceptor::new(ssl, tokio::io::join(clt_r, clt_w)).map_err(|e| {
                TlsInterceptionError::InternalOpensslServerError(anyhow!(
                    "failed to create lazy acceptor: {e}"
                ))
            })?;

        // also use upstream timeout config for client handshake
        let accept_timeout = self.tls_interception.server_config.accept_timeout;

        tokio::time::timeout(accept_timeout, lazy_acceptor.accept())
            .await
            .map_err(|_| TlsInterceptionError::ClientHandshakeTimeout)?
            .map_err(|e| {
                TlsInterceptionError::ClientHandshakeFailed(anyhow!(
                    "read client hello msg failed: {e:?}"
                ))
            })?;

        // build to server ssl context based on client hello
        let sni_hostname = self
            .tls_interception
            .server_config
            .fetch_server_name(lazy_acceptor.ssl());
        if let Some(domain) = sni_hostname {
            // TODO also fetch user-site config here?
            self.upstream.set_host(Host::from(domain));
        }
        let alpn_ext = self
            .tls_interception
            .server_config
            .fetch_alpn_extension(lazy_acceptor.ssl());
        let ups_ssl = match self.ctx.user_site_tls_client() {
            Some(c) => c
                .build_mimic_ssl(sni_hostname, &self.upstream, alpn_ext)
                .map_err(|e| {
                    TlsInterceptionError::UpstreamPrepareFailed(anyhow!(
                        "failed to build user-site ssl context: {e}"
                    ))
                })?,
            None => self
                .tls_interception
                .client_config
                .build_ssl(sni_hostname, &self.upstream, alpn_ext)
                .map_err(|e| {
                    TlsInterceptionError::UpstreamPrepareFailed(anyhow!(
                        "failed to build general ssl context: {e}"
                    ))
                })?,
        };

        // handshake with upstream server
        let ups_tls_connector =
            SslConnector::new(ups_ssl, tokio::io::join(ups_r, ups_w)).map_err(|e| {
                TlsInterceptionError::UpstreamPrepareFailed(anyhow!(
                    "failed to get ssl stream: {e}"
                ))
            })?;
        let ups_tls_stream = tokio::time::timeout(accept_timeout, ups_tls_connector.connect())
            .await
            .map_err(|_| TlsInterceptionError::UpstreamHandshakeTimeout)?
            .map_err(|e| {
                TlsInterceptionError::UpstreamHandshakeFailed(anyhow!(
                    "upstream handshake error: {e}"
                ))
            })?;

        let upstream_cert = ups_tls_stream.ssl().peer_certificate().ok_or_else(|| {
            TlsInterceptionError::NoFakeCertGenerated(anyhow!("failed to get upstream certificate"))
        })?;
        self.server_verify_result = Some(ups_tls_stream.ssl().verify_result());
        let cert_domain = sni_hostname
            .map(|v| v.to_string())
            .unwrap_or_else(|| self.upstream.host().to_string());
        let cert_pair = self
            .tls_interception
            .cert_agent
            .fetch(
                TlsServiceType::from(self.protocol),
                CERT_USAGE,
                Arc::from(cert_domain),
                upstream_cert,
            )
            .await
            .ok_or_else(|| {
                TlsInterceptionError::NoFakeCertGenerated(anyhow!(
                    "failed to get fake upstream certificate"
                ))
            })?;

        // set certificate and private key
        let clt_ssl = lazy_acceptor.ssl_mut();
        cert_pair
            .add_to_ssl(clt_ssl)
            .map_err(TlsInterceptionError::InternalOpensslServerError)?;
        // set alpn
        if let Some(alpn_protocol) = ups_tls_stream.ssl().selected_alpn_protocol() {
            self.tls_interception
                .server_config
                .set_selected_alpn(clt_ssl, alpn_protocol.to_vec());
        }

        let clt_acceptor = lazy_acceptor.into_acceptor();
        let clt_tls_stream = tokio::time::timeout(accept_timeout, clt_acceptor.accept())
            .await
            .map_err(|_| TlsInterceptionError::ClientHandshakeTimeout)?
            .map_err(|e| {
                TlsInterceptionError::ClientHandshakeFailed(anyhow!(
                    "client handshake error: {e:?}"
                ))
            })?;

        let (clt_r, clt_w) = clt_tls_stream.into_split();
        let (ups_r, ups_w) = ups_tls_stream.into_split();

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
            if stream_dumper.client_side() {
                let (clt_r, clt_w) = stream_dumper.wrap_client_io(
                    self.ctx.task_notes.client_addr,
                    self.ctx.task_notes.server_addr,
                    dissector_hint,
                    clt_r,
                    clt_w,
                );
                Ok(self.inspect_inner(protocol, clt_r, clt_w, ups_r, ups_w))
            } else {
                let (ups_r, ups_w) = stream_dumper.wrap_remote_io(
                    self.ctx.task_notes.client_addr,
                    self.ctx.task_notes.server_addr,
                    dissector_hint,
                    ups_r,
                    ups_w,
                );
                Ok(self.inspect_inner(protocol, clt_r, clt_w, ups_r, ups_w))
            }
        } else {
            Ok(self.inspect_inner(protocol, clt_r, clt_w, ups_r, ups_w))
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
        CR: AsyncRead + Send + Unpin + 'static,
        CW: AsyncWrite + Send + Unpin + 'static,
        UR: AsyncRead + Send + Unpin + 'static,
        UW: AsyncWrite + Send + Unpin + 'static,
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
