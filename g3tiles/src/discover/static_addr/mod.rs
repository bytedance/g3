/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::Context;
use tokio::sync::watch;
use yaml_rust::Yaml;

use g3_types::metrics::NodeName;

use super::{ArcDiscoverInternal, Discover, DiscoverInternal, DiscoverResult};
use crate::config::discover::static_addr::StaticAddrDiscoverConfig;
use crate::config::discover::{AnyDiscoverConfig, DiscoverConfig};

pub(crate) struct StaticAddrDiscover {
    config: StaticAddrDiscoverConfig,
}

impl StaticAddrDiscover {
    pub(crate) fn new_obj(config: StaticAddrDiscoverConfig) -> ArcDiscoverInternal {
        Arc::new(StaticAddrDiscover { config })
    }
}

impl Discover for StaticAddrDiscover {
    fn name(&self) -> &NodeName {
        self.config.name()
    }

    fn register_yaml(&self, data: &Yaml) -> anyhow::Result<watch::Receiver<DiscoverResult>> {
        let input = self.config.parse_yaml_data(data).context(format!(
            "invalid input data for discover {}",
            self.config.name()
        ))?;
        let (sender, mut receiver) = watch::channel(Ok(input.inner));
        receiver.mark_changed();
        tokio::spawn(async move { sender.closed().await });
        Ok(receiver)
    }
}

impl DiscoverInternal for StaticAddrDiscover {
    fn _clone_config(&self) -> AnyDiscoverConfig {
        AnyDiscoverConfig::StaticAddr(self.config.clone())
    }

    fn _update_config_in_place(&self, _config: AnyDiscoverConfig) -> anyhow::Result<()> {
        Ok(())
    }
}
