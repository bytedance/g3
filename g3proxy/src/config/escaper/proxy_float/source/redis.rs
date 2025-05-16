/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::{Context, anyhow};
use url::Url;
use yaml_rust::{Yaml, yaml};

use g3_redis_client::RedisClientConfigBuilder;
use g3_types::net::UpstreamAddr;
use g3_yaml::YamlDocPosition;

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(crate) struct ProxyFloatRedisSource {
    pub(crate) client_builder: RedisClientConfigBuilder,
    pub(crate) sets_key: String,
}

impl ProxyFloatRedisSource {
    fn new(server: UpstreamAddr) -> Self {
        ProxyFloatRedisSource {
            client_builder: RedisClientConfigBuilder::new(server),
            sets_key: String::new(),
        }
    }

    pub(super) fn parse_map(
        map: &yaml::Hash,
        position: Option<&YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut config = ProxyFloatRedisSource::default();

        g3_yaml::foreach_kv(map, |k, v| {
            config
                .set(k, v, position)
                .context(format!("failed to parse key {k}"))
        })?;

        config.check()?;
        Ok(config)
    }

    pub(super) fn parse_url(url: &Url, position: Option<&YamlDocPosition>) -> anyhow::Result<Self> {
        if let Some(host) = url.host_str() {
            let port = url.port().unwrap_or(g3_redis_client::REDIS_DEFAULT_PORT);
            let upstream = UpstreamAddr::from_host_str_and_port(host, port)?;
            let mut config = ProxyFloatRedisSource::new(upstream);

            let path = url.path();
            let db_str = path.strip_prefix('/').unwrap_or(path);
            if !db_str.is_empty() {
                let db = i64::from_str(db_str)
                    .map_err(|_| anyhow!("the path should be a valid redis db number"))?;
                config.client_builder.set_db(db);
            }
            let username = url.username();
            if !username.is_empty() {
                config.client_builder.set_username(username.to_string());
            }
            if let Some(password) = url.password() {
                config.client_builder.set_password(password.to_string());
            }

            for (k, v) in url.query_pairs() {
                let yaml_value = Yaml::String(v.to_string());
                config
                    .set(&k, &yaml_value, position)
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

    fn set(&mut self, k: &str, v: &Yaml, position: Option<&YamlDocPosition>) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            super::CONFIG_KEY_SOURCE_TYPE => Ok(()),
            "sets_key" => {
                self.sets_key = g3_yaml::value::as_string(v)?;
                Ok(())
            }
            _ => {
                let lookup_dir = g3_daemon::config::get_lookup_dir(position)?;
                self.client_builder.set_by_yaml_kv(k, v, Some(lookup_dir))
            }
        }
    }
}
