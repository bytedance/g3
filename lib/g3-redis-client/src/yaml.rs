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

use std::path::Path;

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use super::RedisClientConfigBuilder;

impl RedisClientConfigBuilder {
    pub fn set_yaml_kv(
        &mut self,
        k: &str,
        v: &Yaml,
        lookup_dir: Option<&Path>,
    ) -> anyhow::Result<()> {
        match k {
            "addr" | "address" => {
                let addr = g3_yaml::value::as_upstream_addr(v, crate::REDIS_DEFAULT_PORT)
                    .context(format!("invalid upstream address value for key {k}"))?;
                self.set_addr(addr);
                Ok(())
            }
            "tls" | "tls_client" => {
                let tls = g3_yaml::value::as_rustls_client_config_builder(v, lookup_dir).context(
                    format!("invalid rustls tls client config value for key {k}"),
                )?;
                self.set_tls_client(tls);
                Ok(())
            }
            "tls_name" => {
                let name = g3_yaml::value::as_rustls_server_name(v)
                    .context(format!("invalid rustls server name value for key {k}"))?;
                self.set_tls_name(name);
                Ok(())
            }
            "db" => {
                let db =
                    g3_yaml::value::as_i64(v).context(format!("invalid int value for key {k}"))?;
                self.set_db(db);
                Ok(())
            }
            "username" => {
                let username = g3_yaml::value::as_string(v)?;
                self.set_username(username);
                Ok(())
            }
            "password" => {
                let password = g3_yaml::value::as_string(v)?;
                self.set_password(password);
                Ok(())
            }
            "connect_timeout" => {
                let timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                self.set_connect_timeout(timeout);
                Ok(())
            }
            "response_timeout" | "read_timeout" => {
                let timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                self.set_response_timeout(timeout);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {}", k)),
        }
    }
}
