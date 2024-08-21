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

use std::borrow::Cow;
use std::sync::Arc;

use anyhow::anyhow;
use log::debug;
use quinn::{ClientConfig, Connection, ConnectionError, Endpoint, TokioRuntime};
use tokio::sync::oneshot;

use g3_types::net::RustlsQuicClientConfig;

use super::StreamDetourStream;
use crate::config::audit::AuditStreamDetourConfig;

pub(super) struct StreamDetourRequest(pub(super) oneshot::Sender<StreamDetourStream>);

pub(super) struct StreamDetourConnector {
    config: Arc<AuditStreamDetourConfig>,
    tls_client: RustlsQuicClientConfig,
}

impl StreamDetourConnector {
    pub(super) fn new(config: Arc<AuditStreamDetourConfig>) -> anyhow::Result<Self> {
        let tls_client = config.tls_client.build_quic()?;
        Ok(StreamDetourConnector { config, tls_client })
    }

    async fn new_connection(&self) -> anyhow::Result<Connection> {
        let mut peers = tokio::net::lookup_host(self.config.peer_addr.to_string())
            .await
            .map_err(|e| anyhow!("failed to resolve {}: {e}", self.config.peer_addr))?;

        let Some(peer) = peers.next() else {
            return Err(anyhow!("no host resolved for {}", self.config.peer_addr));
        };

        let socket = g3_socket::udp::new_std_socket_to(
            peer,
            None,
            self.config.socket_buffer,
            Default::default(),
        )
        .map_err(|e| anyhow!("failed to setup local udp socket: {e}"))?;
        socket
            .connect(peer)
            .map_err(|e| anyhow!("failed to connect local udp socket to {peer}: {e}"))?;

        let endpoint = Endpoint::new(Default::default(), None, socket, Arc::new(TokioRuntime))
            .map_err(|e| anyhow!("failed to create quic endpoint: {e}"))?;

        let client_config = ClientConfig::new(self.tls_client.driver.clone());
        let tls_name = self
            .config
            .tls_name
            .as_ref()
            .map(Cow::Borrowed)
            .unwrap_or_else(|| Cow::Owned(peer.ip().to_string()));
        let client_connect = endpoint
            .connect_with(client_config, peer, &tls_name)
            .map_err(|e| anyhow!("failed to create quic client: {e}"))?;

        tokio::time::timeout(self.tls_client.handshake_timeout, client_connect)
            .await
            .map_err(|_| anyhow!("quic connect to peer {peer} time out"))?
            .map_err(|e| anyhow!("quic connect to peer {peer} failed: {e}"))
    }

    pub(super) async fn run_new_connection(
        &self,
        req_receiver: flume::Receiver<StreamDetourRequest>,
    ) {
        let mut connection = match self.new_connection().await {
            Ok(c) => c,
            Err(e) => {
                debug!("failed to connect to detour server: {e:?}");
                return;
            }
        };

        let mut reuse_limit = self.config.connection_reuse_limit;

        while reuse_limit > 0 {
            tokio::select! {
                e = connection.closed() => {
                    debug!("detour connection closed by upstream: {e}");
                    break;
                }
                r = req_receiver.recv_async() => {
                    match r {
                        Ok(req) => {
                            if let Err(e) = self.handle_req(req, &mut connection).await {
                                debug!("error when handle new detour request: {e}");
                                break;
                            }
                            reuse_limit -= 1;
                        }
                        Err(_) => break,
                    }
                }
            }
        }
    }

    async fn handle_req(
        &self,
        req: StreamDetourRequest,
        connection: &mut Connection,
    ) -> Result<(), ConnectionError> {
        let c_stream = connection.open_bi().await?;
        let s_stream = connection.open_bi().await?;

        let stream = StreamDetourStream {
            north_send: c_stream.0,
            north_recv: c_stream.1,
            south_send: s_stream.0,
            south_recv: s_stream.1,
        };
        let _ = req.0.send(stream);
        Ok(())
    }
}
