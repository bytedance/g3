/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::path::Path;

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_daemon::log::{LogConfig, LogConfigContainer};
use g3_types::sync::GlobalInit;

static TASK_DEFAULT_LOG_CONFIG_CONTAINER: GlobalInit<LogConfigContainer> =
    GlobalInit::new(LogConfigContainer::new());

pub(crate) fn load(v: &Yaml, conf_dir: &Path) -> anyhow::Result<()> {
    let mut default_log_config: Option<LogConfig> = None;
    match v {
        Yaml::String(s) => {
            let config = LogConfig::with_driver_name(s, crate::build::PKG_NAME)?;
            default_log_config = Some(config);
        }
        Yaml::Hash(map) => {
            g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                "default" => {
                    let config = LogConfig::parse_yaml(v, conf_dir, crate::build::PKG_NAME)
                        .context(format!("invalid value for key {k}"))?;
                    default_log_config = Some(config);
                    Ok(())
                }
                "syslog" => {
                    let config = LogConfig::parse_syslog_yaml(v, crate::build::PKG_NAME)
                        .context(format!("invalid syslog config value for key {k}"))?;
                    default_log_config = Some(config);
                    Ok(())
                }
                "fluentd" => {
                    let config = LogConfig::parse_fluentd_yaml(v, conf_dir, crate::build::PKG_NAME)
                        .context(format!("invalid fluentd config value for key {k}"))?;
                    default_log_config = Some(config);
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
        }
        Yaml::Null => return Ok(()),
        _ => return Err(anyhow!("invalid value type")),
    }
    if let Some(config) = default_log_config {
        TASK_DEFAULT_LOG_CONFIG_CONTAINER.with_mut(|l| l.set_default(config));
    }
    Ok(())
}

pub(crate) fn get_task_default_config() -> LogConfig {
    TASK_DEFAULT_LOG_CONFIG_CONTAINER
        .as_ref()
        .get(crate::build::PKG_NAME)
}
