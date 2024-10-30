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

use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::anyhow;
use log::debug;
use s2n_tls::callbacks::{ClientHelloCallback, ConnectionFuture};
use s2n_tls::config::Config;
use s2n_tls::connection::{Connection, ModifiedBuilder};
use s2n_tls::error::Error;
use s2n_tls_tokio::{TlsAcceptor, TlsStream};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::Instant;

use g3_daemon::stat::task::TcpStreamConnectionStats;
use g3_io_ext::LimitedStream;
use g3_types::limit::GaugeSemaphorePermit;
use g3_types::net::Host;
use g3_types::route::HostMatch;

use super::{CommonTaskContext, S2nTlsRelayTask};
use crate::module::stream::StreamAcceptTaskCltWrapperStats;
use crate::serve::s2n_tls_proxy::S2nTlsHost;

pub(crate) struct S2nTlsAcceptTask {
    ctx: CommonTaskContext,
    alive_permit: Option<GaugeSemaphorePermit>,
}

impl S2nTlsAcceptTask {
    pub(crate) fn new(ctx: CommonTaskContext) -> Self {
        S2nTlsAcceptTask {
            ctx,
            alive_permit: None,
        }
    }

    pub(crate) async fn into_running(
        mut self,
        stream: TcpStream,
        hosts: &Arc<HostMatch<Arc<S2nTlsHost>>>,
        accept_config: Config,
    ) {
        let time_accepted = Instant::now();

        let pre_handshake_stats = Arc::new(TcpStreamConnectionStats::default());
        let wrapper_stats =
            StreamAcceptTaskCltWrapperStats::new(&self.ctx.server_stats, &pre_handshake_stats);

        let limit_config = self.ctx.server_config.tcp_sock_speed_limit;
        let stream = LimitedStream::local_limited(
            stream,
            limit_config.shift_millis,
            limit_config.max_north,
            limit_config.max_south,
            Arc::new(wrapper_stats),
        );

        if let Some((mut tls_stream, host)) = self.handshake(stream, hosts, accept_config).await {
            if tls_stream.as_ref().resumed() {
                // Quick ACK is needed with session resumption
                self.ctx.cc_info.tcp_sock_try_quick_ack();
            }

            let backend = if let Some(alpn) = tls_stream.as_ref().application_protocol() {
                let protocol = unsafe { std::str::from_utf8_unchecked(alpn) };
                host.get_backend(protocol)
            } else {
                host.get_default_backend()
            };
            let Some(backend) = backend else {
                let _ = tls_stream.shutdown().await;
                return;
            };

            S2nTlsRelayTask::new(
                self.ctx,
                host,
                backend.clone(),
                time_accepted.elapsed(),
                pre_handshake_stats,
                self.alive_permit,
            )
            .into_running(tls_stream)
            .await;
        }
    }

    async fn handshake<S>(
        &mut self,
        stream: S,
        hosts: &Arc<HostMatch<Arc<S2nTlsHost>>>,
        accept_config: Config,
    ) -> Option<(TlsStream<S>, Arc<S2nTlsHost>)>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let conn_builder = ModifiedBuilder::new(accept_config, |conn| {
            conn.set_application_context(ClientHelloContext::new(hosts.clone()));
            Ok(conn)
        });
        let tls_acceptor = TlsAcceptor::new(conn_builder);
        match tokio::time::timeout(
            self.ctx.server_config.accept_timeout,
            tls_acceptor.accept(stream),
        )
        .await
        {
            Ok(Ok(mut tls_stream)) => {
                let ctx = tls_stream
                    .as_mut()
                    .application_context_mut::<ClientHelloContext>()?;
                self.alive_permit = ctx.sema.take();

                let host = ctx.host.take()?;
                Some((tls_stream, host))
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

    pub(crate) fn build_accept_config() -> anyhow::Result<Config> {
        let mut builder = Config::builder();
        builder
            .set_security_policy(&s2n_tls::security::DEFAULT_TLS13)
            .map_err(|e| anyhow!("failed to set security policy: {e}"))?;
        builder
            .with_system_certs(false)
            .map_err(|e| anyhow!("failed to disable load of system certs: {e}"))?;
        builder
            .set_client_hello_callback(AcceptClientHelloCallback {})
            .map_err(|e| anyhow!("failed to set client hello callback: {e}"))?;
        builder
            .build()
            .map_err(|e| anyhow!("failed to build accept tls config: {e}"))
    }
}

#[derive(Debug, Error)]
enum ClientHelloApplicationError {
    #[error("rate limited")]
    RateLimited,
    #[error("max alive quota reached")]
    AliveLimited,
}

struct ClientHelloContext {
    hosts: Arc<HostMatch<Arc<S2nTlsHost>>>,
    sema: Option<GaugeSemaphorePermit>,
    host: Option<Arc<S2nTlsHost>>,
}

impl ClientHelloContext {
    fn new(hosts: Arc<HostMatch<Arc<S2nTlsHost>>>) -> Self {
        ClientHelloContext {
            hosts,
            sema: None,
            host: None,
        }
    }
}

struct AcceptClientHelloCallback {}

impl ClientHelloCallback for AcceptClientHelloCallback {
    fn on_client_hello(
        &self,
        connection: &mut Connection,
    ) -> Result<Option<Pin<Box<dyn ConnectionFuture>>>, Error> {
        let ch = connection.client_hello()?;
        let sni = ch.server_name()?;

        let Some(ctx) = connection.application_context_mut::<ClientHelloContext>() else {
            return Ok(None);
        };

        let host = if !sni.is_empty() {
            let server_name = std::str::from_utf8(sni.as_slice()).unwrap();
            let host = Host::from_str(server_name).unwrap();
            ctx.hosts.get(&host)
        } else {
            ctx.hosts.get_default()
        };

        match host {
            Some(host) => {
                ctx.host = Some(host.clone());
                if host.check_rate_limit().is_err() {
                    return Err(Error::application(Box::new(
                        ClientHelloApplicationError::RateLimited,
                    )));
                }
                // we do not check request alive sema here
                let Ok(sema) = host.acquire_request_semaphore() else {
                    return Err(Error::application(Box::new(
                        ClientHelloApplicationError::AliveLimited,
                    )));
                };
                ctx.sema = sema;

                let config = host.tls_config.clone();
                connection.set_config(config)?;
                connection.server_name_extension_used();
                Ok(None)
            }
            None => Ok(None),
        }
    }
}
