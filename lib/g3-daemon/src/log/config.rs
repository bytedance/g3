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
use std::sync::Arc;

use anyhow::{anyhow, Context};
use yaml_rust::Yaml;

use g3_fluentd::FluentdClientConfig;
use g3_syslog::SyslogBuilder;

const DEFAULT_CHANNEL_SIZE: usize = 4096;
const IO_ERROR_SAMPLING_OFFSET_MAX: usize = 16;
const IO_ERROR_SAMPLING_OFFSET_DEFAULT: usize = 10;

#[derive(Clone)]
pub enum LogConfigDriver {
    Discard,
    #[cfg(target_os = "linux")]
    Journal,
    Syslog(SyslogBuilder),
    Fluentd(Arc<FluentdClientConfig>),
}

#[derive(Clone)]
pub struct LogConfig {
    pub(crate) driver: LogConfigDriver,
    pub(crate) async_channel_size: usize,
    pub(crate) async_thread_number: usize,
    pub(crate) io_err_sampling_mask: usize,
    pub(crate) program_name: &'static str,
}

impl LogConfig {
    fn with_driver(driver: LogConfigDriver, program_name: &'static str) -> Self {
        LogConfig {
            driver,
            async_channel_size: DEFAULT_CHANNEL_SIZE,
            async_thread_number: 1,
            io_err_sampling_mask: (1 << IO_ERROR_SAMPLING_OFFSET_DEFAULT) - 1,
            program_name,
        }
    }

    pub fn default_discard(program_name: &'static str) -> Self {
        Self::with_driver(LogConfigDriver::Discard, program_name)
    }

    #[cfg(target_os = "linux")]
    pub fn default_journal(program_name: &'static str) -> Self {
        Self::with_driver(LogConfigDriver::Journal, program_name)
    }

    pub fn default_syslog(program_name: &'static str) -> Self {
        Self::with_driver(
            LogConfigDriver::Syslog(SyslogBuilder::with_ident(program_name.to_string())),
            program_name,
        )
    }

    pub fn default_fluentd(program_name: &'static str) -> Self {
        Self::with_driver(
            LogConfigDriver::Fluentd(Arc::new(FluentdClientConfig::default())),
            program_name,
        )
    }

    pub fn parse(
        v: &Yaml,
        conf_dir: &Path,
        program_name: &'static str,
    ) -> anyhow::Result<LogConfig> {
        match v {
            Yaml::String(s) => match s.as_str() {
                "discard" => Ok(LogConfig::default_discard(program_name)),
                #[cfg(target_os = "linux")]
                "journal" => Ok(LogConfig::default_journal(program_name)),
                "syslog" => Ok(LogConfig::default_syslog(program_name)),
                "fluentd" => Ok(LogConfig::default_fluentd(program_name)),
                _ => Err(anyhow!("invalid log config")),
            },
            Yaml::Hash(map) => {
                let mut config = LogConfig::default_discard(program_name);
                g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                    #[cfg(target_os = "linux")]
                    "journal" => {
                        config.driver = LogConfigDriver::Journal;
                        Ok(())
                    }
                    "syslog" => {
                        let builder =
                            g3_yaml::value::as_syslog_builder(v, program_name.to_string())
                                .context("invalid syslog config")?;
                        config.driver = LogConfigDriver::Syslog(builder);
                        Ok(())
                    }
                    "fluentd" => {
                        let client = g3_yaml::value::as_fluentd_client_config(v, Some(conf_dir))
                            .context("invalid fluentd config")?;
                        config.driver = LogConfigDriver::Fluentd(Arc::new(client));
                        Ok(())
                    }
                    "async_channel_size" | "channel_size" => {
                        let channel_size = g3_yaml::value::as_usize(v)
                            .context(format!("invalid usize value for key {k}"))?;
                        config.async_channel_size = channel_size;
                        Ok(())
                    }
                    "async_thread_number" | "thread_number" => {
                        let thread_number = g3_yaml::value::as_usize(v)
                            .context(format!("invalid usize value for key {k}"))?;
                        config.async_thread_number = thread_number;
                        Ok(())
                    }
                    "io_error_sampling_offset" => {
                        let offset = g3_yaml::value::as_usize(v)
                            .context(format!("invalid value for key {k}"))?;
                        if offset > IO_ERROR_SAMPLING_OFFSET_MAX {
                            Err(anyhow!(
                                "value for {k} should be less than {IO_ERROR_SAMPLING_OFFSET_MAX}"
                            ))
                        } else {
                            config.io_err_sampling_mask = (1 << offset) - 1;
                            Ok(())
                        }
                    }
                    _ => Err(anyhow!("invalid key {k}")),
                })?;
                Ok(config)
            }
            _ => Err(anyhow!("invalid value type")),
        }
    }
}
