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

use g3_daemon::server::{BaseServer, ServerReloadCommand};
use g3_types::metrics::NodeName;

use super::{ArcCollect, Collect, CollectInternal};
use crate::config::collect::dummy::DummyCollectConfig;
use crate::config::collect::{AnyCollectConfig, CollectConfig};

pub(crate) struct DummyCollect {
    config: DummyCollectConfig,
    reload_sender: broadcast::Sender<ServerReloadCommand>,
}

impl DummyCollect {
    fn new(config: DummyCollectConfig) -> Self {
        let reload_sender = crate::collect::new_reload_notify_channel();

        DummyCollect {
            config,
            reload_sender,
        }
    }

    pub(crate) fn prepare_initial(config: DummyCollectConfig) -> anyhow::Result<ArcCollect> {
        let server = DummyCollect::new(config);
        Ok(Arc::new(server))
    }

    pub(crate) fn prepare_default(name: &NodeName) -> ArcCollect {
        let config = DummyCollectConfig::with_name(name, None);
        Arc::new(DummyCollect::new(config))
    }

    fn prepare_reload(&self, config: AnyCollectConfig) -> anyhow::Result<DummyCollect> {
        if let AnyCollectConfig::Dummy(config) = config {
            Ok(DummyCollect::new(config))
        } else {
            Err(anyhow!(
                "config type mismatch: expect {}, actual {}",
                self.config.collect_type(),
                config.collect_type()
            ))
        }
    }
}

impl CollectInternal for DummyCollect {
    fn _clone_config(&self) -> AnyCollectConfig {
        AnyCollectConfig::Dummy(self.config.clone())
    }

    fn _depend_on_collector(&self, _name: &NodeName) -> bool {
        false
    }

    fn _reload_config_notify_runtime(&self) {
        let cmd = ServerReloadCommand::ReloadVersion(0);
        let _ = self.reload_sender.send(cmd);
    }

    fn _update_next_collectors_in_place(&self) {}

    fn _reload_with_old_notifier(&self, config: AnyCollectConfig) -> anyhow::Result<ArcCollect> {
        Err(anyhow!(
            "this {} collect doesn't support reload with old notifier",
            config.collect_type()
        ))
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

impl BaseServer for DummyCollect {
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
        0
    }
}

impl Collect for DummyCollect {}
