/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use tokio::sync::watch;
use yaml_rust::Yaml;

use g3_types::collection::WeightedValue;
use g3_types::metrics::NodeName;

use super::{ArcDiscoverInternal, Discover, DiscoverInternal, DiscoverResult};
use crate::config::discover::host_resolver::HostResolverDiscoverConfig;
use crate::config::discover::{AnyDiscoverConfig, DiscoverConfig};

pub(crate) struct HostResolverDiscover {
    config: HostResolverDiscoverConfig,
}

impl HostResolverDiscover {
    pub(crate) fn new_obj(config: HostResolverDiscoverConfig) -> ArcDiscoverInternal {
        Arc::new(HostResolverDiscover { config })
    }
}

impl Discover for HostResolverDiscover {
    fn name(&self) -> &NodeName {
        self.config.name()
    }

    fn register_yaml(&self, data: &Yaml) -> anyhow::Result<watch::Receiver<DiscoverResult>> {
        let input = self.config.parse_yaml_data(data).context(format!(
            "invalid input data for discover {}",
            self.config.name()
        ))?;
        let (sender, receiver) = watch::channel(Ok(Vec::new()));
        tokio::spawn(async move {
            let addr = input.addr.to_string();
            loop {
                let _ = match tokio::net::lookup_host(&addr).await {
                    Ok(iter) => {
                        let addrs: Vec<_> = iter.into_iter().map(WeightedValue::new).collect();
                        sender.send_replace(Ok(addrs))
                    }
                    Err(e) => sender.send_replace(Err(anyhow::Error::new(e))),
                };
                match tokio::time::timeout(Duration::from_secs(60), sender.closed()).await {
                    Ok(_) => break,
                    Err(_) => continue,
                }
            }
        });
        Ok(receiver)
    }
}

impl DiscoverInternal for HostResolverDiscover {
    fn _clone_config(&self) -> AnyDiscoverConfig {
        AnyDiscoverConfig::HostResolver(self.config.clone())
    }

    fn _update_config_in_place(&self, _config: AnyDiscoverConfig) -> anyhow::Result<()> {
        Ok(())
    }
}
