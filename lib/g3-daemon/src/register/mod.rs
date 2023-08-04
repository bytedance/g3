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

use yaml_rust::Yaml;

mod config;
pub use config::RegisterConfig;

mod task;
pub use task::RegisterTask;

static mut PRE_REGISTER_CONFIG: Option<Arc<RegisterConfig>> = None;

pub fn load_pre_config(v: &Yaml) -> anyhow::Result<()> {
    let mut config = RegisterConfig::default();
    config.parse(v)?;
    unsafe { PRE_REGISTER_CONFIG = Some(Arc::new(config)) }
    Ok(())
}

pub fn get_pre_config() -> Option<Arc<RegisterConfig>> {
    unsafe { PRE_REGISTER_CONFIG.clone() }
}
