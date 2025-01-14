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

use std::str::FromStr;
use std::sync::OnceLock;

use anyhow::anyhow;
use log::warn;
use yaml_rust::Yaml;

use g3_statsd_client::StatsdClientConfig;
use g3_types::metrics::NodeName;

static GLOBAL_STAT_CONFIG: OnceLock<StatsdClientConfig> = OnceLock::new();

pub fn get_global_stat_config() -> Option<StatsdClientConfig> {
    GLOBAL_STAT_CONFIG.get().cloned()
}

fn set_global_stat_config(config: StatsdClientConfig) {
    if GLOBAL_STAT_CONFIG.set(config).is_err() {
        warn!("Global stat config has already been set");
    }
}

pub fn load(v: &Yaml, prefix: &'static str) -> anyhow::Result<()> {
    let prefix =
        NodeName::from_str(prefix).map_err(|e| anyhow!("invalid default metrics prefix: {e}"))?;
    let config = StatsdClientConfig::parse_yaml(v, prefix)?;
    set_global_stat_config(config);
    Ok(())
}
