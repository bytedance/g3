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

use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use tokio::sync::watch;
use yaml_rust::Yaml;

use g3_types::collection::WeightedValue;

use super::{ArcDiscover, Discover, DiscoverResult};
use crate::config::discover::host_resolver::HostResolverDiscoverConfig;
use crate::config::discover::{AnyDiscoverConfig, DiscoverConfig};

pub(crate) struct HostResolverDiscover {
    config: HostResolverDiscoverConfig,
}

impl HostResolverDiscover {
    pub(crate) fn new_obj(config: HostResolverDiscoverConfig) -> ArcDiscover {
        Arc::new(HostResolverDiscover { config })
    }
}

impl Discover for HostResolverDiscover {
    fn _clone_config(&self) -> AnyDiscoverConfig {
        AnyDiscoverConfig::HostResolver(self.config.clone())
    }

    fn _update_config_in_place(&self, _config: AnyDiscoverConfig) -> anyhow::Result<()> {
        Ok(())
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
