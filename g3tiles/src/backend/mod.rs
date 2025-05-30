/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::net::IpAddr;
use std::sync::Arc;

use async_trait::async_trait;

use g3_types::collection::{SelectiveItem, SelectivePickPolicy, SelectiveVec};
use g3_types::metrics::NodeName;

use crate::config::backend::AnyBackendConfig;
use crate::module::keyless::{KeylessRequest, KeylessResponse};
use crate::module::stream::{StreamConnectError, StreamConnectResult};
use crate::serve::ServerTaskNotes;

mod dummy_close;
#[cfg(feature = "quic")]
mod keyless_quic;
mod keyless_tcp;
mod stream_tcp;

mod ops;
pub use ops::load_all;
pub(crate) use ops::{get_backend, reload, update_dependency_to_discover};

mod registry;
use registry::BackendRegistry;
pub(crate) use registry::{get_names, get_or_insert_default};

#[async_trait]
pub(crate) trait Backend {
    fn name(&self) -> &NodeName;

    fn discover(&self) -> &NodeName;
    fn update_discover(&self) -> anyhow::Result<()>;

    fn alive_connection(&self) -> u64;

    async fn stream_connect(&self, _task_notes: &ServerTaskNotes) -> StreamConnectResult {
        Err(StreamConnectError::UpstreamNotResolved) // TODO
    }

    async fn keyless(&self, req: KeylessRequest) -> KeylessResponse {
        KeylessResponse::not_implemented(req.header())
    }
}

trait BackendInternal: Backend {
    fn _clone_config(&self) -> AnyBackendConfig;
    fn _update_config_in_place(&self, flags: u64, config: AnyBackendConfig) -> anyhow::Result<()>;

    fn _reload(
        &self,
        config: AnyBackendConfig,
        registry: &mut BackendRegistry,
    ) -> anyhow::Result<ArcBackendInternal>;
}

pub(crate) type ArcBackend = Arc<dyn Backend + Send + Sync>;
type ArcBackendInternal = Arc<dyn BackendInternal + Send + Sync>;

pub(crate) trait BackendExt: Backend {
    fn select_consistent<'a, T>(
        &self,
        nodes: &'a SelectiveVec<T>,
        pick_policy: SelectivePickPolicy,
        task_notes: &ServerTaskNotes,
    ) -> &'a T
    where
        T: SelectiveItem,
    {
        #[derive(Hash)]
        struct ConsistentKey {
            client_ip: IpAddr,
        }

        match pick_policy {
            SelectivePickPolicy::Random => nodes.pick_random(),
            SelectivePickPolicy::Serial => nodes.pick_serial(),
            SelectivePickPolicy::RoundRobin => nodes.pick_round_robin(),
            SelectivePickPolicy::Ketama => {
                let key = ConsistentKey {
                    client_ip: task_notes.client_ip(),
                };
                nodes.pick_ketama(&key)
            }
            SelectivePickPolicy::Rendezvous => {
                let key = ConsistentKey {
                    client_ip: task_notes.client_ip(),
                };
                nodes.pick_rendezvous(&key)
            }
            SelectivePickPolicy::JumpHash => {
                let key = ConsistentKey {
                    client_ip: task_notes.client_ip(),
                };
                nodes.pick_jump(&key)
            }
        }
    }
}
