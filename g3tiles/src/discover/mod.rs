/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::anyhow;
use tokio::sync::watch;
use yaml_rust::Yaml;

use g3_types::collection::WeightedValue;
use g3_types::metrics::NodeName;

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
    fn name(&self) -> &NodeName;

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

trait DiscoverInternal: Discover {
    fn _clone_config(&self) -> AnyDiscoverConfig;
    fn _update_config_in_place(&self, config: AnyDiscoverConfig) -> anyhow::Result<()>;
}

pub(crate) type ArcDiscover = Arc<dyn Discover + Send + Sync>;
type ArcDiscoverInternal = Arc<dyn DiscoverInternal + Send + Sync>;
