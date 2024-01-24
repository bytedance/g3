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
use openssl::ssl::{Ssl, SslVersion};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::Instant;

use g3_daemon::stat::task::TcpStreamConnectionStats;
use g3_io_ext::LimitedStream;
use g3_openssl::{SslAcceptor, SslLazyAcceptor, SslStream};
use g3_types::limit::GaugeSemaphorePermit;

use super::{CommonTaskContext, OpensslRelayTask};

mod stats;
use stats::OpensslAcceptTaskCltWrapperStats;

pub(crate) struct OpensslAcceptTask {
    ctx: CommonTaskContext,
    alive_permit: Option<GaugeSemaphorePermit>,
}

impl OpensslAcceptTask {
    pub(crate) fn new(ctx: CommonTaskContext) -> Self {
        OpensslAcceptTask {
            ctx,
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
            Ok(mut ssl_stream) => {
                let Some(host) = ssl_stream.ssl_mut().ex_data_mut(self.ctx.host_index) else {
                    return;
                };
                let Some(host) = host.take() else {
                    return;
                };

                if let Some(sema) = ssl_stream
                    .ssl_mut()
                    .ex_data_mut(self.ctx.alive_permit_index)
                {
                    // This is not necessary, as the ex_data will also be dropped.
                    // But we can drop it early by taking it out
                    self.alive_permit = sema.take();
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
                debug!("{e}");
            }
        }
    }

    async fn handshake<S>(&mut self, stream: S, ssl: Ssl) -> anyhow::Result<SslStream<S>>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let mut lazy_acceptor = SslLazyAcceptor::new(ssl, stream).unwrap();
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

        let Some(client_hello_version) = lazy_acceptor
            .ssl()
            .ex_data(self.ctx.client_hello_version_index)
            .copied()
        else {
            return Err(anyhow!("no client hello version found"));
        };
        let acceptor = self.get_acceptor(lazy_acceptor, client_hello_version)?;

        match tokio::time::timeout(self.ctx.server_config.accept_timeout, acceptor.accept()).await {
            Ok(Ok(ssl_stream)) => Ok(ssl_stream),
            Ok(Err(e)) => {
                // TODO free host and sema
                Err(anyhow!("failed to accept ssl handshake: {e}"))
            }
            Err(_) => {
                // TODO free host and sema
                Err(anyhow!("timeout to accept ssl handshake"))
            }
        }
    }

    #[cfg(feature = "vendored-tongsuo")]
    fn get_acceptor<S>(
        &self,
        lazy_acceptor: SslLazyAcceptor<S>,
        client_hello_version: SslVersion,
    ) -> anyhow::Result<SslAcceptor<S>>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        if client_hello_version == SslVersion::NTLS1_1 {
            lazy_acceptor
                .into_acceptor(&self.ctx.tlcp_context)
                .map_err(|e| anyhow!("failed to set tlcp context: {e}"))
        } else {
            lazy_acceptor
                .into_acceptor(&self.ctx.ssl_context)
                .map_err(|e| anyhow!("failed to set tlcp context: {e}"))
        }
    }

    #[cfg(not(feature = "vendored-tongsuo"))]
    fn get_acceptor<S>(
        &self,
        lazy_acceptor: SslLazyAcceptor<S>,
        _client_hello_version: SslVersion,
    ) -> anyhow::Result<SslAcceptor<S>>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        lazy_acceptor
            .into_acceptor(&self.ctx.ssl_context)
            .map_err(|e| anyhow!("failed to set tlcp context: {e}"))
    }
}
