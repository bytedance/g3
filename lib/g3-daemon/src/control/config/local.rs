/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use yaml_rust::Yaml;

use g3_types::sync::GlobalInit;

use super::GeneralControllerConfig;

pub(crate) struct LocalControllerConfig {
    general: GeneralControllerConfig,
}

static LOCAL_CONTROLLER_CONFIG: GlobalInit<LocalControllerConfig> =
    GlobalInit::new(LocalControllerConfig {
        general: GeneralControllerConfig::new(),
    });

impl LocalControllerConfig {
    pub(crate) fn get_general() -> GeneralControllerConfig {
        LOCAL_CONTROLLER_CONFIG.as_ref().general.clone()
    }

    pub(crate) fn set_default(v: &Yaml) -> anyhow::Result<()> {
        match v {
            Yaml::Hash(map) => {
                g3_yaml::foreach_kv(map, |k, v| {
                    LOCAL_CONTROLLER_CONFIG.with_mut(|config| config.set(k, v))
                })?;
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
