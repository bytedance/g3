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

const CONFIG_KEY_BACKEND_TYPE: &str = "type";

static BACKEND_CONFIG: GlobalInit<BackendConfig> = GlobalInit::new(BackendConfig::Simple);

pub(crate) enum BackendConfig {
    Simple,
    #[cfg(feature = "openssl-async-job")]
    AsyncJob(AsyncJobBackendConfig),
}

pub(super) fn load(value: &Yaml) -> anyhow::Result<()> {
    match value {
        Yaml::Hash(map) => {
            let backend_type = g3_yaml::hash_get_required_str(map, CONFIG_KEY_BACKEND_TYPE)?;
            match g3_yaml::key::normalize(backend_type).as_str() {
                "simple" => Ok(()),
                #[cfg(feature = "openssl-async-job")]
                "async_job" | "openssl_async_job" => {
                    let config = AsyncJobBackendConfig::parse_yaml(map)?;
                    BACKEND_CONFIG.set(BackendConfig::AsyncJob(config));
                    Ok(())
                }
                _ => Err(anyhow!("unsupported backend type {backend_type}")),
            }
        }
        Yaml::String(s) => match g3_yaml::key::normalize(s).as_str() {
            "simple" => Ok(()),
            #[cfg(feature = "openssl-async-job")]
            "async_job" | "openssl_async_job" => {
                let config = AsyncJobBackendConfig::default();
                BACKEND_CONFIG.set(BackendConfig::AsyncJob(config));
                Ok(())
            }
            _ => Err(anyhow!("unsupported backend type {s}")),
        },
        _ => Err(anyhow!("invalid yaml value type")),
    }
}

pub(crate) fn get_config() -> &'static BackendConfig {
    BACKEND_CONFIG.as_ref()
}
