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

use std::path::Path;

use anyhow::{anyhow, Context};
use yaml_rust::Yaml;

use g3_daemon::log::{LogConfig, LogConfigContainer};

static mut RESOLVE_DEFAULT_LOG_CONFIG_CONTAINER: LogConfigContainer = LogConfigContainer::new();
static mut ESCAPE_DEFAULT_LOG_CONFIG_CONTAINER: LogConfigContainer = LogConfigContainer::new();
static mut AUDIT_DEFAULT_LOG_CONFIG_CONTAINER: LogConfigContainer = LogConfigContainer::new();
static mut TASK_DEFAULT_LOG_CONFIG_CONTAINER: LogConfigContainer = LogConfigContainer::new();

pub(crate) fn load(v: &Yaml, conf_dir: &Path) -> anyhow::Result<()> {
    match v {
        Yaml::String(s) => {
            let config = match s.as_str() {
                "discard" => LogConfig::default_discard(crate::build::PKG_NAME),
                "journal" => LogConfig::default_journal(crate::build::PKG_NAME),
                "syslog" => LogConfig::default_syslog(crate::build::PKG_NAME),
                "fluentd" => LogConfig::default_fluentd(crate::build::PKG_NAME),
                _ => return Err(anyhow!("invalid default log config")),
            };
            unsafe {
                RESOLVE_DEFAULT_LOG_CONFIG_CONTAINER.set_default(config.clone());
                ESCAPE_DEFAULT_LOG_CONFIG_CONTAINER.set_default(config.clone());
                AUDIT_DEFAULT_LOG_CONFIG_CONTAINER.set_default(config.clone());
                TASK_DEFAULT_LOG_CONFIG_CONTAINER.set_default(config);
            }
            Ok(())
        }
        Yaml::Hash(map) => {
            g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                "default" => {
                    let config = LogConfig::parse(v, conf_dir, crate::build::PKG_NAME)
                        .context(format!("invalid value for key {k}"))?;
                    unsafe {
                        RESOLVE_DEFAULT_LOG_CONFIG_CONTAINER.set_default(config.clone());
                        ESCAPE_DEFAULT_LOG_CONFIG_CONTAINER.set_default(config.clone());
                        AUDIT_DEFAULT_LOG_CONFIG_CONTAINER.set_default(config.clone());
                        TASK_DEFAULT_LOG_CONFIG_CONTAINER.set_default(config);
                    }
                    Ok(())
                }
                "resolve" => {
                    let config = LogConfig::parse(v, conf_dir, crate::build::PKG_NAME)
                        .context(format!("invalid value for key {k}"))?;
                    unsafe {
                        RESOLVE_DEFAULT_LOG_CONFIG_CONTAINER.set(config);
                    }
                    Ok(())
                }
                "escape" => {
                    let config = LogConfig::parse(v, conf_dir, crate::build::PKG_NAME)
                        .context(format!("invalid value for key {k}"))?;
                    unsafe {
                        ESCAPE_DEFAULT_LOG_CONFIG_CONTAINER.set(config);
                    }
                    Ok(())
                }
                "audit" => {
                    let config = LogConfig::parse(v, conf_dir, crate::build::PKG_NAME)
                        .context(format!("invalid value for key {k}"))?;
                    unsafe {
                        AUDIT_DEFAULT_LOG_CONFIG_CONTAINER.set_default(config);
                    }
                    Ok(())
                }
                "task" => {
                    let config = LogConfig::parse(v, conf_dir, crate::build::PKG_NAME)
                        .context(format!("invalid value for key {k}"))?;
                    unsafe {
                        TASK_DEFAULT_LOG_CONFIG_CONTAINER.set(config);
                    }
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

pub(crate) fn get_resolve_default_config() -> LogConfig {
    unsafe { RESOLVE_DEFAULT_LOG_CONFIG_CONTAINER.get(crate::build::PKG_NAME) }
}

pub(crate) fn get_escape_default_config() -> LogConfig {
    unsafe { ESCAPE_DEFAULT_LOG_CONFIG_CONTAINER.get(crate::build::PKG_NAME) }
}

pub(crate) fn get_audit_default_config() -> LogConfig {
    unsafe { AUDIT_DEFAULT_LOG_CONFIG_CONTAINER.get(crate::build::PKG_NAME) }
}

pub(crate) fn get_task_default_config() -> LogConfig {
    unsafe { TASK_DEFAULT_LOG_CONFIG_CONTAINER.get(crate::build::PKG_NAME) }
}
