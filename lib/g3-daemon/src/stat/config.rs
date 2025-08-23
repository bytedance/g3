/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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
