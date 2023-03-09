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

use std::str::FromStr;
use std::time::Duration;

use anyhow::{anyhow, Context};
use redis::{ConnectionAddr, ConnectionInfo, IntoConnectionInfo, RedisConnectionInfo, RedisResult};
use url::Url;
use yaml_rust::{yaml, Yaml};

use g3_types::net::UpstreamAddr;

const CONFIG_KEY_SOURCE_ADDR: &str = "addr";

const REDIS_DEFAULT_PORT: u16 = 6379;
const REDIS_DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REDIS_DEFAULT_READ_TIMEOUT: Duration = Duration::from_secs(2);

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProxyFloatRedisSource {
    addr: UpstreamAddr,
    db: i64,
    username: Option<String>,
    password: Option<String>,
    pub(crate) connect_timeout: Duration,
    pub(crate) read_timeout: Duration,
    pub(crate) sets_key: String,
}

impl ProxyFloatRedisSource {
    fn new(addr: UpstreamAddr) -> Self {
        ProxyFloatRedisSource {
            addr,
            db: 0,
            password: None,
            username: None,
            connect_timeout: REDIS_DEFAULT_CONNECT_TIMEOUT,
            read_timeout: REDIS_DEFAULT_READ_TIMEOUT,
            sets_key: String::new(),
        }
    }

    pub(super) fn parse_map(map: &yaml::Hash) -> anyhow::Result<Self> {
        let v = g3_yaml::hash_get_required(map, CONFIG_KEY_SOURCE_ADDR)?;
        let upstream = g3_yaml::value::as_upstream_addr(v, REDIS_DEFAULT_PORT).context(format!(
            "invalid upstream addr value for key {CONFIG_KEY_SOURCE_ADDR}"
        ))?;
        let mut config = ProxyFloatRedisSource::new(upstream);

        g3_yaml::foreach_kv(map, |k, v| {
            config.set(k, v).context(format!("failed to parse key {k}"))
        })?;

        config.check()?;
        Ok(config)
    }

    pub(super) fn parse_url(url: &Url) -> anyhow::Result<Self> {
        if let Some(host) = url.host_str() {
            let port = url.port().unwrap_or(REDIS_DEFAULT_PORT);
            let upstream = UpstreamAddr::from_host_str_and_port(host, port)?;
            let mut config = ProxyFloatRedisSource::new(upstream);

            let path = url.path();
            let db_str = path.strip_prefix('/').unwrap_or(path);
            if !db_str.is_empty() {
                let db = i64::from_str(db_str)
                    .map_err(|_| anyhow!("the path should be a valid redis db number"))?;
                config.db = db;
            }
            let username = url.username();
            if !username.is_empty() {
                config.username = Some(username.to_string());
            }
            if let Some(password) = url.password() {
                config.password = Some(password.to_string());
            }

            for (k, v) in url.query_pairs() {
                let yaml_value = Yaml::String(v.to_string());
                config
                    .set(&k, &yaml_value)
                    .context(format!("failed to parse query param {k}={v}"))?;
            }

            config.check()?;
            Ok(config)
        } else {
            Err(anyhow!("no host set"))
        }
    }

    fn check(&self) -> anyhow::Result<()> {
        if self.sets_key.is_empty() {
            return Err(anyhow!("no sets name set"));
        }
        Ok(())
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            super::CONFIG_KEY_SOURCE_TYPE => Ok(()),
            CONFIG_KEY_SOURCE_ADDR => Ok(()),
            "db" => {
                self.db =
                    g3_yaml::value::as_i64(v).context(format!("invalid int value for key {k}"))?;
                Ok(())
            }
            "username" => {
                let username = g3_yaml::value::as_string(v)?;
                self.username = Some(username);
                Ok(())
            }
            "password" => {
                let password = g3_yaml::value::as_string(v)?;
                self.password = Some(password);
                Ok(())
            }
            "connect_timeout" => {
                self.connect_timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "read_timeout" => {
                self.read_timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "sets_key" => {
                self.sets_key = g3_yaml::value::as_string(v)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {}", k)),
        }
    }
}

impl IntoConnectionInfo for &ProxyFloatRedisSource {
    fn into_connection_info(self) -> RedisResult<ConnectionInfo> {
        Ok(ConnectionInfo {
            addr: ConnectionAddr::Tcp(self.addr.host().to_string(), self.addr.port()),
            redis: RedisConnectionInfo {
                db: self.db,
                username: self.username.clone(),
                password: self.password.clone(),
            },
        })
    }
}
