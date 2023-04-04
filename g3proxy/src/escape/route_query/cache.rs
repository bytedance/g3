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

use std::hash::Hash;
use std::net::IpAddr;
use std::sync::Arc;

use anyhow::anyhow;
use tokio::net::UdpSocket;

use g3_io_ext::EffectiveCacheHandle;
use g3_types::collection::{SelectivePickPolicy, SelectiveVec, WeightedValue};
use g3_types::metrics::MetricsName;
use g3_types::net::UpstreamAddr;

use super::query::QueryRuntime;
use super::RouteQueryEscaperConfig;
use crate::serve::ServerTaskNotes;

#[derive(Clone, Debug, Hash, PartialEq, PartialOrd, Ord, Eq)]
pub(super) struct CacheQueryKey {
    pub(super) user: String,
    pub(super) host: String,
    pub(super) client_ip: Option<String>,
}

#[derive(Hash)]
struct CacheQueryConsistentKey {
    client_ip: IpAddr,
}

pub(super) struct CacheHandle {
    inner: EffectiveCacheHandle<CacheQueryKey, SelectiveVec<WeightedValue<MetricsName>>>,
}

impl CacheHandle {
    pub(super) async fn select(
        &self,
        config: &RouteQueryEscaperConfig,
        task_notes: &ServerTaskNotes,
        upstream: &UpstreamAddr,
    ) -> Option<MetricsName> {
        let client_ip = if config.query_pass_client_ip {
            Some(task_notes.client_addr.ip().to_string())
        } else {
            None
        };
        let query_key = CacheQueryKey {
            user: task_notes
                .user_ctx()
                .map(|ctx| ctx.user().name().to_string())
                .unwrap_or_default(),
            host: upstream.host().to_string(),
            client_ip,
        };

        self.inner
            .fetch(Arc::new(query_key), config.cache_request_timeout)
            .await
            .and_then(|r| {
                if let Some(nodes) = r.inner() {
                    let node = match config.cache_pick_policy {
                        SelectivePickPolicy::Random => nodes.pick_random(),
                        SelectivePickPolicy::Serial => nodes.pick_serial(),
                        SelectivePickPolicy::RoundRobin => nodes.pick_round_robin(),
                        SelectivePickPolicy::Rendezvous => {
                            let select_key = CacheQueryConsistentKey {
                                client_ip: task_notes.client_addr.ip(),
                            };
                            nodes.pick_rendezvous(&select_key)
                        }
                        SelectivePickPolicy::JumpHash => {
                            let select_key = CacheQueryConsistentKey {
                                client_ip: task_notes.client_addr.ip(),
                            };
                            nodes.pick_jump(&select_key)
                        }
                    };
                    Some(node.inner().clone())
                } else {
                    None
                }
            })
    }
}

pub(super) async fn spawn(config: &Arc<RouteQueryEscaperConfig>) -> anyhow::Result<CacheHandle> {
    use anyhow::Context;

    let (socket, _addr) =
        g3_socket::udp::new_std_bind_connect(None, config.query_socket_buffer, &Default::default())
            .context("failed to setup udp socket")?;
    socket.connect(config.query_peer_addr).map_err(|e| {
        anyhow!(
            "failed to connect to peer address {}: {e:?}",
            config.query_peer_addr
        )
    })?;
    let socket = UdpSocket::from_std(socket).context("failed to setup udp socket")?;

    let (cache_runtime, cache_handle, query_handle) =
        g3_io_ext::spawn_effective_cache(config.cache_request_batch_count);
    let query_runtime = QueryRuntime::new(config, socket, query_handle);

    tokio::spawn(query_runtime);
    tokio::spawn(cache_runtime);

    Ok(CacheHandle {
        inner: cache_handle,
    })
}
