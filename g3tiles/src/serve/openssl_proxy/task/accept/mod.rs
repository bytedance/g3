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

use std::pin::Pin;
use std::sync::Arc;

use log::debug;
use openssl::ex_data::Index;
use openssl::ssl::Ssl;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::Instant;
use tokio_openssl::SslStream;

use g3_daemon::stat::task::TcpStreamConnectionStats;
use g3_io_ext::LimitedStream;

use super::{CommonTaskContext, OpensslRelayTask};
use crate::serve::openssl_proxy::host::OpensslHost;

mod stats;
use stats::OpensslAcceptTaskCltWrapperStats;

pub(crate) struct OpensslAcceptTask {
    ctx: CommonTaskContext,
    host_index: Index<Ssl, Arc<OpensslHost>>,
}

impl OpensslAcceptTask {
    pub(crate) fn new(ctx: CommonTaskContext, host_index: Index<Ssl, Arc<OpensslHost>>) -> Self {
        OpensslAcceptTask { ctx, host_index }
    }

    pub(crate) async fn into_running(self, stream: TcpStream, ssl: Ssl) {
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

        if let Some(mut ssl_stream) = self.handshake(stream, ssl).await {
            if let Some(host) = ssl_stream.ssl().ex_data(self.host_index) {
                let service = if let Some(alpn) = ssl_stream.ssl().selected_alpn_protocol() {
                    let protocol = unsafe { std::str::from_utf8_unchecked(alpn) };
                    host.services.get(protocol)
                } else {
                    host.services.get_default()
                };
                let Some(service) = service else {
                    let _ = ssl_stream.shutdown().await;
                    return;
                };

                OpensslRelayTask::new(
                    self.ctx,
                    Arc::clone(host),
                    Arc::clone(service),
                    time_accepted.elapsed(),
                    pre_handshake_stats,
                )
                .into_running(ssl_stream)
                .await;
            } else {
                unreachable!()
            }
        }
    }

    async fn handshake<S>(&self, stream: S, ssl: Ssl) -> Option<SslStream<S>>
    where
        S: AsyncRead + AsyncWrite + Unpin,
    {
        let mut ssl_stream = SslStream::new(ssl, stream).unwrap();
        match tokio::time::timeout(
            self.ctx.server_config.accept_timeout,
            Pin::new(&mut ssl_stream).accept(),
        )
        .await
        {
            Ok(Ok(_)) => Some(ssl_stream),
            Ok(Err(e)) => {
                debug!("failed to accept ssl handshake: {e}");
                None
            }
            Err(_) => {
                debug!("timeout to accept ssl handshake");
                None
            }
        }
    }
}
