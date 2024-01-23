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

use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;

use g3_types::metrics::MetricsName;

use super::{ArcBackend, Backend};
use crate::config::backend::dummy_close::DummyCloseBackendConfig;
use crate::config::backend::{AnyBackendConfig, BackendConfig};
use crate::module::stream::{StreamConnectError, StreamConnectResult};
use crate::serve::ServerTaskNotes;

pub(crate) struct DummyCloseBackend {
    config: DummyCloseBackendConfig,
}

impl DummyCloseBackend {
    fn new_obj(config: DummyCloseBackendConfig) -> ArcBackend {
        Arc::new(DummyCloseBackend { config })
    }

    pub(super) fn prepare_initial(config: DummyCloseBackendConfig) -> anyhow::Result<ArcBackend> {
        Ok(DummyCloseBackend::new_obj(config))
    }

    pub(super) fn prepare_default(name: &MetricsName) -> ArcBackend {
        let config = DummyCloseBackendConfig::new(name, None);
        DummyCloseBackend::new_obj(config)
    }

    fn prepare_reload(config: DummyCloseBackendConfig) -> anyhow::Result<ArcBackend> {
        Ok(DummyCloseBackend::new_obj(config))
    }
}

#[async_trait]
impl Backend for DummyCloseBackend {
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

    async fn _lock_safe_reload(&self, config: AnyBackendConfig) -> anyhow::Result<ArcBackend> {
        if let AnyBackendConfig::DummyClose(c) = config {
            // TODO add stats
            DummyCloseBackend::prepare_reload(c)
        } else {
            Err(anyhow!("invalid backend config type"))
        }
    }

    #[inline]
    fn name(&self) -> &MetricsName {
        self.config.name()
    }

    fn discover(&self) -> &MetricsName {
        Default::default()
    }
    fn update_discover(&self) -> anyhow::Result<()> {
        Ok(())
    }

    async fn stream_connect(&self, _task_notes: &ServerTaskNotes) -> StreamConnectResult {
        Err(StreamConnectError::UpstreamNotResolved)
    }
}
