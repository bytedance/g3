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
use log::debug;
use openssl::ssl::{Ssl, SslContext, SslRef};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::Instant;

use g3_daemon::stat::task::TcpStreamConnectionStats;
use g3_io_ext::LimitedStream;
use g3_openssl::SslStream;
use g3_types::limit::GaugeSemaphorePermit;
use g3_types::route::HostMatch;

use super::{CommonTaskContext, OpensslRelayTask};

mod stats;
use crate::serve::openssl_proxy::host::OpensslHost;
use stats::OpensslAcceptTaskCltWrapperStats;

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

    pub(crate) async fn into_running(mut self, stream: TcpStream, ssl: Ssl) {
        let time_accepted = Instant::now();

        let pre_handshake_stats = Arc::new(TcpStreamConnectionStats::default());
        let wrapper_stats =
            OpensslAcceptTaskCltWrapperStats::new(&self.ctx.server_stats, &pre_handshake_stats);

        let limit_config = self.ctx.server_config.tcp_sock_speed_limit;
        let stream = LimitedStream::new(
            stream,
            limit_config.shift_millis,
            limit_config.max_north,
            limit_config.max_south,
            Arc::new(wrapper_stats),
        );

        match self.handshake(stream, ssl).await {
            Ok((mut ssl_stream, host)) => {
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
                debug!("{e}");
            }
        }
    }

    async fn handshake<S>(
        &mut self,
        stream: S,
        ssl: Ssl,
    ) -> anyhow::Result<(SslStream<S>, Arc<OpensslHost>)>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let mut lazy_acceptor = g3_openssl::SslLazyAcceptor::new(ssl, stream).unwrap();
        match tokio::time::timeout(
            self.ctx.server_config.client_hello_recv_timeout,
            lazy_acceptor.accept(),
        )
        .await
        {
            Ok(Ok(_)) => {}
            Ok(Err(e)) => {
                return Err(anyhow!("failed to recv ssl client hello: {e}"));
            }
            Err(_) => {
                return Err(anyhow!("timeout to recv ssl client hello"));
            }
        }

        let Some(host) = self.get_host(lazy_acceptor.ssl()) else {
            return Err(anyhow!("no matched host config found"));
        };
        #[cfg(feature = "vendored-tongsuo")]
        let host_ssl_context = self.get_host_ssl_context(lazy_acceptor.ssl(), &host)?;
        #[cfg(not(feature = "vendored-tongsuo"))]
        let host_ssl_context = self.get_host_ssl_context(&host);
        let Some(ssl_context) = host_ssl_context else {
            return Err(anyhow!("no matched host ssl context found"));
        };

        host.check_rate_limit()
            .map_err(|_| anyhow!("host level rate limit reached"))?;
        self.alive_permit = host
            .acquire_request_semaphore()
            .map_err(|_| anyhow!("host level alive limit reached"))?;

        let acceptor = lazy_acceptor
            .into_acceptor(ssl_context)
            .map_err(|e| anyhow!("failed to set final ssl context: {e}"))?;

        match tokio::time::timeout(self.ctx.server_config.accept_timeout, acceptor.accept()).await {
            Ok(Ok(ssl_stream)) => Ok((ssl_stream, host)),
            Ok(Err(e)) => Err(anyhow!("failed to accept ssl handshake: {e}")),
            Err(_) => Err(anyhow!("timeout to accept ssl handshake")),
        }
    }

    fn get_host(&self, lazy_ssl: &SslRef) -> Option<Arc<OpensslHost>> {
        if let Some(host) = lazy_ssl.ex_data(self.ctx.host_name_index) {
            self.hosts.get(host).cloned()
        } else {
            self.hosts.get_default().cloned()
        }
    }

    #[cfg(feature = "vendored-tongsuo")]
    fn get_host_ssl_context<'b>(
        &self,
        lazy_ssl: &SslRef,
        host: &'b Arc<OpensslHost>,
    ) -> anyhow::Result<Option<&'b SslContext>> {
        use openssl::ssl::SslVersion;

        let Some(client_hello_version) = lazy_ssl
            .ex_data(self.ctx.client_hello_version_index)
            .copied()
        else {
            return Err(anyhow!("no client hello version found"));
        };
        let host_ssl_context = if client_hello_version == SslVersion::NTLS1_1 {
            host.tlcp_context.as_ref()
        } else {
            host.ssl_context.as_ref()
        };
        Ok(host_ssl_context)
    }

    #[cfg(not(feature = "vendored-tongsuo"))]
    fn get_host_ssl_context<'b>(&self, host: &'b Arc<OpensslHost>) -> Option<&'b SslContext> {
        host.ssl_context.as_ref()
    }
}
