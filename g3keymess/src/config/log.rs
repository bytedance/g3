/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::path::Path;

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_daemon::log::{LogConfig, LogConfigContainer};
use g3_types::sync::GlobalInit;

static REQUEST_DEFAULT_LOG_CONFIG_CONTAINER: GlobalInit<LogConfigContainer> =
    GlobalInit::new(LogConfigContainer::new());
static TASK_DEFAULT_LOG_CONFIG_CONTAINER: GlobalInit<LogConfigContainer> =
    GlobalInit::new(LogConfigContainer::new());

pub(crate) fn load(v: &Yaml, conf_dir: &Path) -> anyhow::Result<()> {
    match v {
        Yaml::String(s) => {
            let config = LogConfig::with_driver_name(s, crate::build::PKG_NAME)?;
            REQUEST_DEFAULT_LOG_CONFIG_CONTAINER.with_mut(|l| l.set_default(config.clone()));
            TASK_DEFAULT_LOG_CONFIG_CONTAINER.with_mut(|l| l.set_default(config));
            Ok(())
        }
        Yaml::Hash(map) => {
            g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                "default" => {
                    let config = LogConfig::parse_yaml(v, conf_dir, crate::build::PKG_NAME)
                        .context(format!("invalid value for key {k}"))?;
                    REQUEST_DEFAULT_LOG_CONFIG_CONTAINER
                        .with_mut(|l| l.set_default(config.clone()));
                    TASK_DEFAULT_LOG_CONFIG_CONTAINER.with_mut(|l| l.set_default(config));
                    Ok(())
                }
                "request" => {
                    let config = LogConfig::parse_yaml(v, conf_dir, crate::build::PKG_NAME)
                        .context(format!("invalid value for key {k}"))?;
                    REQUEST_DEFAULT_LOG_CONFIG_CONTAINER.with_mut(|l| l.set(config));
                    Ok(())
                }
                "task" => {
                    let config = LogConfig::parse_yaml(v, conf_dir, crate::build::PKG_NAME)
                        .context(format!("invalid value for key {k}"))?;
                    TASK_DEFAULT_LOG_CONFIG_CONTAINER.with_mut(|l| l.set(config));
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;
            Ok(())
        }
        Yaml::Null => Ok(()),
        _ => Err(anyhow!("invalid value type")),
    }
}

pub(crate) fn get_request_default_config() -> LogConfig {
    REQUEST_DEFAULT_LOG_CONFIG_CONTAINER
        .as_ref()
        .get(crate::build::PKG_NAME)
}

pub(crate) fn get_task_default_config() -> LogConfig {
    TASK_DEFAULT_LOG_CONFIG_CONTAINER
        .as_ref()
        .get(crate::build::PKG_NAME)
}
