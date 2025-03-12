/*
 * Copyright 2025 ByteDance and/or its affiliates.
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
use tokio::sync::broadcast;

use g3_daemon::server::BaseServer;
use g3_types::metrics::NodeName;

use super::InternalEmitter;
use crate::collect::{ArcCollect, Collect, CollectInternal};
use crate::config::collect::internal::InternalCollectConfig;
use crate::config::collect::{AnyCollectConfig, CollectConfig};

pub(crate) struct InternalCollect {
    config: Arc<InternalCollectConfig>,

    reload_sender: broadcast::Sender<Arc<InternalCollectConfig>>,
    reload_version: usize,
}

impl InternalCollect {
    fn new(
        config: InternalCollectConfig,
        reload_sender: broadcast::Sender<Arc<InternalCollectConfig>>,
        reload_version: usize,
    ) -> Self {
        InternalCollect {
            config: Arc::new(config),
            reload_sender,
            reload_version,
        }
    }

    pub(crate) fn prepare_initial(config: InternalCollectConfig) -> anyhow::Result<ArcCollect> {
        let server = InternalCollect::new(config, broadcast::Sender::new(4), 1);
        let emitter = InternalEmitter::new(server.reload_sender.subscribe());
        let config = server.config.clone();
        tokio::spawn(emitter.into_running(config));
        Ok(Arc::new(server))
    }

    fn prepare_reload(&self, config: AnyCollectConfig) -> anyhow::Result<InternalCollect> {
        if let AnyCollectConfig::Internal(config) = config {
            Ok(InternalCollect::new(
                config,
                self.reload_sender.clone(),
                self.reload_version + 1,
            ))
        } else {
            Err(anyhow!(
                "config type mismatch: expect {}, actual {}",
                self.config.collect_type(),
                config.collect_type()
            ))
        }
    }
}

impl CollectInternal for InternalCollect {
    fn _clone_config(&self) -> AnyCollectConfig {
        AnyCollectConfig::Internal((*self.config).clone())
    }

    fn _depend_on_collector(&self, _name: &NodeName) -> bool {
        false
    }

    fn _reload_config_notify_runtime(&self) {
        let _ = self.reload_sender.send(self.config.clone());
    }

    fn _update_next_collectors_in_place(&self) {}

    fn _reload_with_old_notifier(&self, config: AnyCollectConfig) -> anyhow::Result<ArcCollect> {
        let mut server = self.prepare_reload(config)?;
        server.reload_sender = self.reload_sender.clone();
        Ok(Arc::new(server))
    }

    fn _reload_with_new_notifier(&self, config: AnyCollectConfig) -> anyhow::Result<ArcCollect> {
        let server = self.prepare_reload(config)?;
        Ok(Arc::new(server))
    }

    fn _start_runtime(&self, _input: &ArcCollect) -> anyhow::Result<()> {
        Ok(())
    }

    fn _abort_runtime(&self) {}
}

impl BaseServer for InternalCollect {
    #[inline]
    fn name(&self) -> &NodeName {
        self.config.name()
    }

    #[inline]
    fn server_type(&self) -> &'static str {
        self.config.collect_type()
    }

    #[inline]
    fn version(&self) -> usize {
        self.reload_version
    }
}

impl Collect for InternalCollect {}
