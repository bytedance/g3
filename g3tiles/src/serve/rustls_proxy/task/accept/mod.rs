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

use std::str::FromStr;
use std::sync::Arc;

use log::debug;
use rustls::server::{Acceptor, ClientHello};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::Instant;
use tokio_rustls::{LazyConfigAcceptor, TlsStream};

use g3_daemon::stat::task::TcpStreamConnectionStats;
use g3_io_ext::LimitedStream;
use g3_types::limit::GaugeSemaphorePermit;
use g3_types::net::Host;
use g3_types::route::HostMatch;

use super::{CommonTaskContext, RustlsRelayTask};
use crate::serve::rustls_proxy::RustlsHost;

mod stats;
use stats::RustlsAcceptTaskCltWrapperStats;

pub(crate) struct RustlsAcceptTask {
    ctx: CommonTaskContext,
    alive_permit: Option<GaugeSemaphorePermit>,
}

impl RustlsAcceptTask {
    pub(crate) fn new(ctx: CommonTaskContext) -> Self {
        RustlsAcceptTask {
            ctx,
            alive_permit: None,
        }
    }

    pub(crate) async fn into_running(
        mut self,
        stream: TcpStream,
        hosts: &HostMatch<Arc<RustlsHost>>,
    ) {
        let time_accepted = Instant::now();

        let pre_handshake_stats = Arc::new(TcpStreamConnectionStats::default());
        let wrapper_stats =
            RustlsAcceptTaskCltWrapperStats::new(&self.ctx.server_stats, &pre_handshake_stats);

        let limit_config = self.ctx.server_config.tcp_sock_speed_limit;
        let stream = LimitedStream::new(
            stream,
            limit_config.shift_millis,
            limit_config.max_north,
            limit_config.max_south,
            Arc::new(wrapper_stats),
        );

        if let Some((mut tls_stream, host)) = self.handshake(stream, hosts).await {
            let service = if let Some(alpn) = tls_stream.get_ref().1.alpn_protocol() {
                let protocol = unsafe { std::str::from_utf8_unchecked(alpn) };
                host.services.get(protocol)
            } else {
                host.services.get_default()
            };

            let Some(service) = service.cloned() else {
                let _ = tls_stream.shutdown().await;
                return;
            };

            RustlsRelayTask::new(
                self.ctx,
                host,
                service,
                time_accepted.elapsed(),
                pre_handshake_stats,
            )
            .into_running(tls_stream)
            .await;
        }
    }

    async fn handshake<S>(
        &mut self,
        stream: S,
        hosts: &HostMatch<Arc<RustlsHost>>,
    ) -> Option<(TlsStream<S>, Arc<RustlsHost>)>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let lazy_acceptor = LazyConfigAcceptor::new(Acceptor::default(), stream);
        match tokio::time::timeout(
            self.ctx.server_config.client_hello_recv_timeout,
            lazy_acceptor,
        )
        .await
        {
            Ok(Ok(d)) => {
                let client_hello = d.client_hello();

                let Some(host) = self.get_host(&client_hello, hosts) else {
                    return None;
                };

                if host.check_rate_limit().is_err() {
                    return None;
                }
                // we do not check request alive sema here
                let Ok(sema) = host.acquire_request_semaphore() else {
                    return None;
                };
                self.alive_permit = sema;

                let accept = d.into_stream(host.tls_config.clone());
                match tokio::time::timeout(host.config.accept_timeout, accept).await {
                    Ok(Ok(s)) => Some((TlsStream::Server(s), host)),
                    Ok(Err(e)) => {
                        debug!("failed to accept tls handshake: {e}");
                        None
                    }
                    Err(_) => {
                        debug!("timeout to accept tls handshake");
                        None
                    }
                }
            }
            Ok(Err(e)) => {
                debug!("failed to recv client hello: {e}");
                None
            }
            Err(_) => {
                debug!("timeout to recv client hello");
                None
            }
        }
    }

    fn get_host(
        &self,
        client_hello: &ClientHello,
        hosts: &HostMatch<Arc<RustlsHost>>,
    ) -> Option<Arc<RustlsHost>> {
        if let Some(sni) = client_hello.server_name() {
            match Host::from_str(sni) {
                Ok(name) => {
                    if let Some(host) = hosts.get(&name) {
                        return Some(host.clone());
                    }
                }
                Err(e) => {
                    debug!("invalid sni hostname: {e:?}");
                    return None;
                }
            }
        }

        hosts.get_default().cloned()
    }
}
