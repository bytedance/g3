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

use yaml_rust::Yaml;

use g3_statsd::client::StatsdClientConfig;
use g3_types::metrics::MetricsName;

static mut GLOBAL_STAT_CONFIG: Option<StatsdClientConfig> = None;

pub fn get_global_stat_config() -> Option<StatsdClientConfig> {
    unsafe { GLOBAL_STAT_CONFIG.clone() }
}

fn set_global_stat_config(config: StatsdClientConfig) {
    unsafe { GLOBAL_STAT_CONFIG = Some(config) }
}

pub fn load(v: &Yaml, prefix: &'static str) -> anyhow::Result<()> {
    let prefix = MetricsName::from_str(prefix).unwrap();
    let config = g3_yaml::value::as_statsd_client_config(v, prefix)?;
    set_global_stat_config(config);
    Ok(())
}
