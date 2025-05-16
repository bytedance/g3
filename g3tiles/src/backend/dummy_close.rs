/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;

use g3_types::metrics::NodeName;

use super::{ArcBackendInternal, Backend, BackendInternal, BackendRegistry};
use crate::config::backend::dummy_close::DummyCloseBackendConfig;
use crate::config::backend::{AnyBackendConfig, BackendConfig};
use crate::module::stream::{StreamConnectError, StreamConnectResult};
use crate::serve::ServerTaskNotes;

pub(crate) struct DummyCloseBackend {
    config: DummyCloseBackendConfig,
}

impl DummyCloseBackend {
    fn new_obj(config: DummyCloseBackendConfig) -> ArcBackendInternal {
        Arc::new(DummyCloseBackend { config })
    }

    pub(super) fn prepare_initial(
        config: DummyCloseBackendConfig,
    ) -> anyhow::Result<ArcBackendInternal> {
        Ok(DummyCloseBackend::new_obj(config))
    }

    pub(super) fn prepare_default(name: &NodeName) -> ArcBackendInternal {
        let config = DummyCloseBackendConfig::new(name, None);
        DummyCloseBackend::new_obj(config)
    }

    fn prepare_reload(config: DummyCloseBackendConfig) -> anyhow::Result<ArcBackendInternal> {
        Ok(DummyCloseBackend::new_obj(config))
    }
}

#[async_trait]
impl Backend for DummyCloseBackend {
    #[inline]
    fn name(&self) -> &NodeName {
        self.config.name()
    }

    fn discover(&self) -> &NodeName {
        Default::default()
    }
    fn update_discover(&self) -> anyhow::Result<()> {
        Ok(())
    }

    fn alive_connection(&self) -> u64 {
        0
    }

    async fn stream_connect(&self, _task_notes: &ServerTaskNotes) -> StreamConnectResult {
        Err(StreamConnectError::UpstreamNotResolved)
    }
}

impl BackendInternal for DummyCloseBackend {
    fn _clone_config(&self) -> AnyBackendConfig {
        AnyBackendConfig::DummyClose(self.config.clone())
    }

    fn _update_config_in_place(
        &self,
        _flags: u64,
        _config: AnyBackendConfig,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    fn _reload(
        &self,
        config: AnyBackendConfig,
        _registry: &mut BackendRegistry,
    ) -> anyhow::Result<ArcBackendInternal> {
        if let AnyBackendConfig::DummyClose(c) = config {
            // TODO add stats
            DummyCloseBackend::prepare_reload(c)
        } else {
            Err(anyhow!("invalid backend config type"))
        }
    }
}
