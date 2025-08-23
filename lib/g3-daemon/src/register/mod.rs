/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::{Arc, OnceLock};

use log::warn;
use yaml_rust::Yaml;

mod config;
pub use config::RegisterConfig;

mod task;
pub use task::RegisterTask;

static PRE_REGISTER_CONFIG: OnceLock<Arc<RegisterConfig>> = OnceLock::new();

pub fn load_pre_config(v: &Yaml) -> anyhow::Result<()> {
    let mut config = RegisterConfig::default();
    config.parse(v)?;
    if PRE_REGISTER_CONFIG.set(Arc::new(config)).is_err() {
        warn!("global register config has already been set");
    }
    Ok(())
}

pub fn get_pre_config() -> Option<Arc<RegisterConfig>> {
    PRE_REGISTER_CONFIG.get().cloned()
}
