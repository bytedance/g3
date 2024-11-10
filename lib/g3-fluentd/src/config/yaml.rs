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

use super::FluentdClientConfig;

impl FluentdClientConfig {
    pub fn parse_yaml(value: &Yaml, lookup_dir: Option<&Path>) -> anyhow::Result<Self> {
        match value {
            Yaml::Hash(map) => {
                let mut config = FluentdClientConfig::default();

                g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                    "address" | "addr" => {
                        let addr = g3_yaml::value::as_env_sockaddr(v)?;
                        config.set_server_addr(addr);
                        Ok(())
                    }
                    "bind_ip" | "bind" => {
                        let ip = g3_yaml::value::as_ipaddr(v)?;
                        config.set_bind_ip(ip);
                        Ok(())
                    }
                    "shared_key" => {
                        let key = g3_yaml::value::as_string(v)?;
                        config.set_shared_key(key);
                        Ok(())
                    }
                    "username" => {
                        let name = g3_yaml::value::as_string(v)?;
                        config.set_username(name);
                        Ok(())
                    }
                    "password" => {
                        let pass = g3_yaml::value::as_string(v)?;
                        config.set_password(pass);
                        Ok(())
                    }
                    "hostname" => {
                        let hostname = g3_yaml::value::as_string(v)?;
                        config.set_hostname(hostname);
                        Ok(())
                    }
                    "tcp_keepalive" => {
                        let keepalive = g3_yaml::value::as_tcp_keepalive_config(v)
                            .context(format!("invalid tcp keepalive config value for key {k}"))?;
                        config.set_tcp_keepalive(keepalive);
                        Ok(())
                    }
                    "tls" | "tls_client" => {
                        let tls_config =
                            g3_yaml::value::as_rustls_client_config_builder(v, lookup_dir)
                                .context(format!(
                                    "invalid rustls tls client config value for key {k}"
                                ))?;
                        config
                            .set_tls_client(tls_config)
                            .context("failed to set tls client config")?;
                        Ok(())
                    }
                    "tls_name" => {
                        let tls_name = g3_yaml::value::as_rustls_server_name(v)
                            .context(format!("invalid rustls server name value for key {k}"))?;
                        config.set_tls_name(tls_name);
                        Ok(())
                    }
                    "connect_timeout" => {
                        let timeout = g3_yaml::humanize::as_duration(v)
                            .context(format!("invalid humanize duration value for key {k}"))?;
                        config.set_connect_timeout(timeout);
                        Ok(())
                    }
                    "connect_delay" => {
                        let delay = g3_yaml::humanize::as_duration(v)
                            .context(format!("invalid humanize duration value for key {k}"))?;
                        config.set_connect_delay(delay);
                        Ok(())
                    }
                    "write_timeout" => {
                        let timeout = g3_yaml::humanize::as_duration(v)
                            .context(format!("invalid humanize duration value for key {k}"))?;
                        config.set_write_timeout(timeout);
                        Ok(())
                    }
                    "flush_interval" => {
                        let interval = g3_yaml::humanize::as_duration(v)
                            .context(format!("invalid humanize duration value for key {k}"))?;
                        config.set_flush_interval(interval);
                        Ok(())
                    }
                    _ => Err(anyhow!("invalid key {k}")),
                })?;

                Ok(config)
            }
            Yaml::String(_) => {
                let addr = g3_yaml::value::as_env_sockaddr(value)?;
                let config = FluentdClientConfig::new(addr);
                Ok(config)
            }
            Yaml::Null => {
                let config = FluentdClientConfig::default();
                Ok(config)
            }
            _ => Err(anyhow!(
                "yaml value type for 'FluentdConfig' should be 'map'"
            )),
        }
    }
}
