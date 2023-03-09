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

use anyhow::anyhow;
use yaml_rust::Yaml;

use super::GeneralControllerConfig;

pub(crate) struct LocalControllerConfig {
    general: GeneralControllerConfig,
}

static mut LOCAL_CONTROLLER_CONFIG: LocalControllerConfig = LocalControllerConfig {
    general: GeneralControllerConfig::new(),
};

impl LocalControllerConfig {
    pub(crate) fn get_general() -> GeneralControllerConfig {
        unsafe { LOCAL_CONTROLLER_CONFIG.general.clone() }
    }

    pub(crate) fn set_default(v: &Yaml) -> anyhow::Result<()> {
        match v {
            Yaml::Hash(map) => {
                g3_yaml::foreach_kv(map, |k, v| unsafe { LOCAL_CONTROLLER_CONFIG.set(k, v) })?;
                Ok(())
            }
            Yaml::Null => Ok(()),
            _ => Err(anyhow!("root value type should be hash")),
        }
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            "recv_timeout" | "send_timeout" => self.general.set(k, v),
            _ => Err(anyhow!("invalid key {k}")),
        }
    }
}
