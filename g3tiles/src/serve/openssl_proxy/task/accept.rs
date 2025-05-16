/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::anyhow;
use bytes::BytesMut;
use log::debug;
use openssl::error::ErrorStack;
use openssl::ssl::{Ssl, SslContext};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::Instant;

use g3_daemon::stat::task::TcpStreamConnectionStats;
use g3_dpi::parser::tls::{
    ClientHello, ExtensionType, HandshakeCoalescer, RawVersion, Record, RecordParseError,
};
use g3_io_ext::{LimitedStream, OnceBufReader};
use g3_openssl::{SslAcceptor, SslStream};
use g3_types::limit::GaugeSemaphorePermit;
use g3_types::net::{Host, TlsServerName};
use g3_types::route::HostMatch;

use super::{CommonTaskContext, OpensslRelayTask};
use crate::module::stream::StreamAcceptTaskCltWrapperStats;
use crate::serve::openssl_proxy::OpensslHost;

pub(crate) struct OpensslAcceptTask {
    ctx: CommonTaskContext,
    hosts: Arc<HostMatch<Arc<OpensslHost>>>,
    alive_permit: Option<GaugeSemaphorePermit>,
}

impl OpensslAcceptTask {
    pub(crate) fn new(ctx: CommonTaskContext, hosts: Arc<HostMatch<Arc<OpensslHost>>>) -> Self {
        OpensslAcceptTask {
            ctx,
            hosts,
            alive_permit: None,
        }
    }

    pub(crate) async fn into_running(mut self, stream: TcpStream) {
        let time_accepted = Instant::now();

        let pre_handshake_stats = Arc::new(TcpStreamConnectionStats::default());
        let wrapper_stats =
            StreamAcceptTaskCltWrapperStats::new(&self.ctx.server_stats, &pre_handshake_stats);

        let limit_config = self.ctx.server_config.tcp_sock_speed_limit;
        let mut stream = LimitedStream::local_limited(
            stream,
            limit_config.shift_millis,
            limit_config.max_north,
            limit_config.max_south,
            Arc::new(wrapper_stats),
        );

        let mut clt_r_buf = BytesMut::with_capacity(2048);
        match self.read_client_hello(&mut stream, &mut clt_r_buf).await {
            Ok((legacy_version, host)) => {
                let mut ssl_stream = match self
                    .handshake(&host, legacy_version, OnceBufReader::new(stream, clt_r_buf))
                    .await
                {
                    Ok(stream) => stream,
                    Err(e) => {
                        debug!("handshake with client failed: {e}");
                        return;
                    }
                };

                if ssl_stream.ssl().session_reused() {
                    // Quick ACK is needed with session resumption
                    self.ctx.cc_info.tcp_sock_try_quick_ack();
                }

                let backend = if let Some(alpn) = ssl_stream.ssl().selected_alpn_protocol() {
                    let protocol = unsafe { std::str::from_utf8_unchecked(alpn) };
                    host.get_backend(protocol)
                } else {
                    host.get_default_backend()
                };
                let Some(backend) = backend else {
                    let _ = ssl_stream.shutdown().await;
                    return;
                };

                OpensslRelayTask::new(
                    self.ctx,
                    host,
                    backend,
                    time_accepted.elapsed(),
                    pre_handshake_stats,
                    self.alive_permit,
                )
                .into_running(ssl_stream)
                .await;
            }
            Err(e) => {
                debug!("dropped connection: {e}")
            }
        };
    }

    async fn read_client_hello<R>(
        &mut self,
        clt_r: &mut R,
        clt_r_buf: &mut BytesMut,
    ) -> anyhow::Result<(RawVersion, Arc<OpensslHost>)>
    where
        R: AsyncRead + Unpin,
    {
        tokio::time::timeout(
            self.ctx.server_config.client_hello_recv_timeout,
            self.do_read_client_hello(clt_r, clt_r_buf),
        )
        .await
        .map_err(|_| anyhow!("timed out to recv client hello message"))?
    }

    async fn do_read_client_hello<R>(
        &mut self,
        clt_r: &mut R,
        clt_r_buf: &mut BytesMut,
    ) -> anyhow::Result<(RawVersion, Arc<OpensslHost>)>
    where
        R: AsyncRead + Unpin,
    {
        let mut handshake_coalescer =
            HandshakeCoalescer::new(self.ctx.server_config.client_hello_max_size);
        let mut record_offset = 0;
        loop {
            let mut record = match Record::parse(&clt_r_buf[record_offset..]) {
                Ok(r) => r,
                Err(RecordParseError::NeedMoreData(_)) => match clt_r.read_buf(clt_r_buf).await {
                    Ok(0) => return Err(anyhow!("connection closed by client")),
                    Ok(_) => continue,
                    Err(e) => return Err(anyhow!("client read error: {e}")),
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
                    return self.parse_sni(ch);
                }
                Ok(None) => match handshake_coalescer.parse_client_hello() {
                    Ok(Some(ch)) => return self.parse_sni(ch),
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

    fn parse_sni(&mut self, ch: ClientHello<'_>) -> anyhow::Result<(RawVersion, Arc<OpensslHost>)> {
        match ch.get_ext(ExtensionType::ServerName) {
            Ok(Some(data)) => {
                let sni = TlsServerName::from_extension_value(data)
                    .map_err(|_| anyhow!("invalid server name in tls client hello message"))?;
                let host = Host::from(sni);
                let Some(host) = self.hosts.get(&host) else {
                    return Err(anyhow!("no tls config found for server named {host}"));
                };
                Ok((ch.legacy_version, host.clone()))
            }
            Ok(None) => match self.hosts.get_default() {
                Some(host) => Ok((ch.legacy_version, host.clone())),
                None => Err(anyhow!("no server name in client hello message")),
            },
            Err(_) => Err(anyhow!("invalid extension in tls client hello request")),
        }
    }

    async fn handshake<S>(
        &mut self,
        host: &OpensslHost,
        legacy_version: RawVersion,
        stream: S,
    ) -> anyhow::Result<SslStream<S>>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        host.check_rate_limit()
            .map_err(|_| anyhow!("host level rate limit reached"))?;
        self.alive_permit = host
            .acquire_request_semaphore()
            .map_err(|_| anyhow!("host level alive limit reached"))?;

        let ssl_context = if legacy_version.is_tlcp() {
            #[cfg(not(feature = "vendored-tongsuo"))]
            return Err(anyhow!("tlcp protocol is not supported"));
            #[cfg(feature = "vendored-tongsuo")]
            host.tlcp_context.as_ref()
        } else {
            host.ssl_context.as_ref()
        };
        let Some(ssl_context) = ssl_context else {
            return Err(anyhow!(
                "no supported tls context for legacy protocol {:?}",
                legacy_version
            ));
        };

        let ssl = self
            .build_ssl(ssl_context)
            .map_err(|e| anyhow!("failed to create SSL instance: {e}"))?;
        let acceptor = SslAcceptor::new(ssl, stream, self.ctx.server_config.accept_timeout)
            .map_err(|e| anyhow!("failed to create new ssl acceptor: {e}"))?;

        acceptor
            .accept()
            .await
            .map_err(|e| anyhow!("failed to accept ssl handshake: {e}"))
    }

    #[cfg(not(feature = "openssl-async-job"))]
    fn build_ssl(&self, ssl_ctx: &SslContext) -> Result<Ssl, ErrorStack> {
        Ssl::new(ssl_ctx)
    }

    #[cfg(feature = "openssl-async-job")]
    fn build_ssl(&self, ssl_ctx: &SslContext) -> Result<Ssl, ErrorStack> {
        use openssl::ssl::SslMode;
        use tokio::runtime::{Handle, RuntimeFlavor};

        let mut ssl = Ssl::new(ssl_ctx)?;
        if self.ctx.server_config.tls_no_async_mode {
            return Ok(ssl);
        }
        if Handle::current().runtime_flavor() == RuntimeFlavor::CurrentThread {
            ssl.set_mode(SslMode::ASYNC);
        }
        Ok(ssl)
    }
}
