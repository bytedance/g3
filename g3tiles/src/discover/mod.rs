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

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::anyhow;
use tokio::sync::watch;
use yaml_rust::Yaml;

use g3_types::collection::WeightedValue;

use crate::config::discover::{AnyDiscoverConfig, DiscoverRegisterData};

mod host_resolver;
mod static_addr;

mod ops;
pub use ops::load_all;
pub(crate) use ops::{get_discover, reload};

mod registry;
pub(crate) use registry::get_names;

pub(crate) type DiscoveredData = Vec<WeightedValue<SocketAddr>>;
pub(crate) type DiscoverResult = anyhow::Result<DiscoveredData>;

pub(crate) trait Discover {
    fn _clone_config(&self) -> AnyDiscoverConfig;
    fn _update_config_in_place(&self, config: AnyDiscoverConfig) -> anyhow::Result<()>;

    fn register_yaml(&self, data: &Yaml) -> anyhow::Result<watch::Receiver<DiscoverResult>>;

    fn register_data(
        &self,
        data: &DiscoverRegisterData,
    ) -> anyhow::Result<watch::Receiver<DiscoverResult>> {
        match data {
            DiscoverRegisterData::Null => Err(anyhow!("no valid register data found")),
            DiscoverRegisterData::Yaml(v) => self.register_yaml(v),
            DiscoverRegisterData::Json(_) => {
                todo!()
            }
        }
    }
}

pub(crate) type ArcDiscover = Arc<dyn Discover + Send + Sync>;
