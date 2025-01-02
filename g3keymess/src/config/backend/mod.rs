/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use anyhow::anyhow;
use yaml_rust::Yaml;

use g3_types::sync::GlobalInit;

#[cfg(feature = "openssl-async-job")]
mod async_job;
#[cfg(feature = "openssl-async-job")]
pub(crate) use async_job::AsyncJobBackendConfig;

static BACKEND_CONFIG: GlobalInit<BackendConfig> =
    GlobalInit::new(BackendConfig::with_driver(BackendDriverConfig::Simple));

pub(crate) struct BackendConfig {
    pub(crate) dispatch_channel_size: usize,
    pub(crate) dispatch_counter_shift: u8,
    pub(crate) driver: BackendDriverConfig,
}

impl Default for BackendConfig {
    fn default() -> Self {
        BackendConfig::with_driver(BackendDriverConfig::Simple)
    }
}

impl BackendConfig {
    const fn with_driver(driver: BackendDriverConfig) -> Self {
        BackendConfig {
            dispatch_channel_size: 1024,
            dispatch_counter_shift: 3,
            driver,
        }
    }
}

pub(crate) enum BackendDriverConfig {
    Simple,
    #[cfg(feature = "openssl-async-job")]
    AsyncJob(AsyncJobBackendConfig),
}

pub(super) fn load(value: &Yaml) -> anyhow::Result<()> {
    let mut config = BackendConfig::default();
    match value {
        Yaml::Hash(map) => {
            g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                "dispatch_channel_size" => {
                    config.dispatch_channel_size = g3_yaml::value::as_usize(v)?;
                    Ok(())
                }
                "dispatch_counter_shift" => {
                    config.dispatch_counter_shift = g3_yaml::value::as_u8(v)?;
                    Ok(())
                }
                #[cfg(feature = "openssl-async-job")]
                "async_job" | "openssl_async_job" => {
                    let driver = AsyncJobBackendConfig::parse_yaml(v)?;
                    config.driver = BackendDriverConfig::AsyncJob(driver);
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;
        }
        Yaml::String(s) => match g3_yaml::key::normalize(s).as_str() {
            "simple" => {}
            #[cfg(feature = "openssl-async-job")]
            "async_job" | "openssl_async_job" => {
                config.driver = BackendDriverConfig::AsyncJob(AsyncJobBackendConfig::default());
            }
            _ => return Err(anyhow!("unsupported backend type {s}")),
        },
        _ => return Err(anyhow!("invalid yaml value type")),
    }
    BACKEND_CONFIG.set(config);
    Ok(())
}

pub(crate) fn get_config() -> &'static BackendConfig {
    BACKEND_CONFIG.as_ref()
}
