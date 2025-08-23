/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::hash::Hash;
use std::net::IpAddr;
use std::sync::Arc;

use anyhow::{Context, anyhow};
use tokio::net::UdpSocket;

use g3_io_ext::EffectiveCacheHandle;
use g3_types::collection::{SelectivePickPolicy, SelectiveVec, WeightedValue};
use g3_types::metrics::NodeName;
use g3_types::net::UpstreamAddr;

use super::RouteQueryEscaperConfig;
use super::query::QueryRuntime;
use crate::serve::ServerTaskNotes;

#[derive(Clone, Debug, Hash, PartialEq, PartialOrd, Ord, Eq)]
pub(super) struct CacheQueryKey {
    pub(super) user: Arc<str>,
    pub(super) host: String,
    pub(super) client_ip: Option<String>,
}

#[derive(Hash)]
struct CacheQueryConsistentKey {
    client_ip: IpAddr,
}

pub(super) struct CacheHandle {
    inner: EffectiveCacheHandle<CacheQueryKey, SelectiveVec<WeightedValue<NodeName>>>,
}

impl CacheHandle {
    pub(super) async fn select(
        &self,
        config: &RouteQueryEscaperConfig,
        task_notes: &ServerTaskNotes,
        upstream: &UpstreamAddr,
    ) -> Option<NodeName> {
        let client_ip = if config.query_pass_client_ip {
            Some(task_notes.client_ip().to_string())
        } else {
            None
        };
        let query_key = CacheQueryKey {
            user: task_notes.raw_user_name().cloned().unwrap_or_default(),
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
                        SelectivePickPolicy::Ketama => {
                            let select_key = CacheQueryConsistentKey {
                                client_ip: task_notes.client_ip(),
                            };
                            nodes.pick_ketama(&select_key)
                        }
                        SelectivePickPolicy::Rendezvous => {
                            let select_key = CacheQueryConsistentKey {
                                client_ip: task_notes.client_ip(),
                            };
                            nodes.pick_rendezvous(&select_key)
                        }
                        SelectivePickPolicy::JumpHash => {
                            let select_key = CacheQueryConsistentKey {
                                client_ip: task_notes.client_ip(),
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

pub(super) fn spawn(config: &Arc<RouteQueryEscaperConfig>) -> anyhow::Result<CacheHandle> {
    let socket = g3_socket::udp::new_std_socket_to(
        config.query_peer_addr,
        &Default::default(),
        config.query_socket_buffer,
        Default::default(),
    )
    .context("failed to setup udp socket")?;
    socket.connect(config.query_peer_addr).map_err(|e| {
        anyhow!(
            "failed to connect to peer address {}: {e:?}",
            config.query_peer_addr
        )
    })?;
    let socket = UdpSocket::from_std(socket).context("failed to setup udp socket")?;

    let (cache_runtime, cache_handle, query_handle) =
        g3_io_ext::create_effective_cache(config.cache_request_batch_count);
    let query_runtime = QueryRuntime::new(config, socket, query_handle);

    tokio::spawn(query_runtime);
    tokio::spawn(cache_runtime);

    Ok(CacheHandle {
        inner: cache_handle,
    })
}
