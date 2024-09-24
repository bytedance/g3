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
