/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, anyhow};
use arcstr::ArcStr;
use yaml_rust::{Yaml, yaml};

use g3_types::net::{ConnectionPoolConfig, OpensslClientConfigBuilder, UpstreamAddr};
use g3_yaml::YamlDocPosition;

use super::{BasicUserGroupConfig, UserGroupConfig};
use crate::config::auth::UserConfig;

const USER_GROUP_TYPE: &str = "ldap";

#[derive(Clone)]
pub(crate) struct LdapUserGroupConfig {
    basic: BasicUserGroupConfig,
    pub(crate) server: UpstreamAddr,
    pub(crate) tls_client: Option<OpensslClientConfigBuilder>,
    pub(crate) direct_tls: bool,
    pub(crate) base_dn: ArcStr,
    pub(crate) unmanaged_user: Option<Arc<UserConfig>>,
    pub(crate) max_message_size: usize,
    pub(crate) connect_timeout: Duration,
    pub(crate) response_timeout: Duration,
    pub(crate) connection_pool: ConnectionPoolConfig,
    pub(crate) queue_channel_size: usize,
    pub(crate) queue_wait_timeout: Duration,
}

impl LdapUserGroupConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        LdapUserGroupConfig {
            basic: BasicUserGroupConfig::new(position),
            server: UpstreamAddr::empty(),
            tls_client: None,
            direct_tls: false,
            base_dn: ArcStr::new(),
            unmanaged_user: None,
            max_message_size: 256,
            connect_timeout: Duration::from_secs(4),
            response_timeout: Duration::from_secs(2),
            connection_pool: ConnectionPoolConfig::new(1024, 8),
            queue_channel_size: 64,
            queue_wait_timeout: Duration::from_secs(4),
        }
    }

    pub(crate) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut config = Self::new(position);
        g3_yaml::foreach_kv(map, |k, v| config.set(k, v))?;
        config.check()?;
        Ok(config)
    }

    fn check(&mut self) -> anyhow::Result<()> {
        if self.server.is_empty() {
            return Err(anyhow!("no ldap url set"));
        }

        if self.direct_tls && self.tls_client.is_none() {
            self.tls_client = Some(OpensslClientConfigBuilder::with_cache_for_one_site());
        }

        self.basic.check()
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            "ldap_url" => {
                let url = g3_yaml::value::as_url(v)
                    .context(format!("invalid ldap url value for key {k}"))?;
                let default_port;
                match url.scheme() {
                    "ldap" => default_port = 389,
                    "ldaps" => {
                        self.direct_tls = true;
                        default_port = 636;
                    }
                    scheme => return Err(anyhow!("unsupported ldap url scheme {scheme}")),
                }
                let Some(host) = url.host() else {
                    return Err(anyhow!("no host found in ldap url {url}"));
                };
                let port = url.port().unwrap_or(default_port);
                self.server = UpstreamAddr::new(host, port);
                let path = url.path();
                let encoded_dn = path.strip_prefix("/").unwrap_or(path);
                let base_dn = percent_encoding::percent_decode_str(encoded_dn)
                    .decode_utf8()
                    .map_err(|e| anyhow!("the base dn is not valid utf-8 string: {e}"))?;
                self.base_dn = ArcStr::from(base_dn.as_ref());
                Ok(())
            }
            "tls_client" => {
                let lookup_dir = g3_daemon::config::get_lookup_dir(self.basic.position.as_ref())?;
                let config = g3_yaml::value::as_to_one_openssl_tls_client_config_builder(
                    v,
                    Some(lookup_dir),
                )
                .context(format!(
                    "invalid openssl tls client config value for key {k}"
                ))?;
                self.tls_client = Some(config);
                Ok(())
            }
            "unmanaged_user" => {
                if let Yaml::Hash(map) = v {
                    let mut user = UserConfig::parse_yaml(map, self.basic.position.as_ref())?;
                    user.set_no_password();
                    self.unmanaged_user = Some(Arc::new(user));
                    Ok(())
                } else {
                    Err(anyhow!("invalid hash value for key {k}"))
                }
            }
            "max_message_size" => {
                self.max_message_size = g3_yaml::value::as_usize(v)?;
                Ok(())
            }
            "connect_timeout" => {
                self.connect_timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "response_timeout" => {
                self.response_timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "connection_pool" | "pool" => {
                self.connection_pool = g3_yaml::value::as_connection_pool_config(v)
                    .context(format!("invalid connection pool config for key {k}"))?;
                Ok(())
            }
            "queue_channel_size" => {
                let channel_size = g3_yaml::value::as_nonzero_usize(v)?;
                self.queue_channel_size = channel_size.get();
                Ok(())
            }
            "queue_wait_timeout" => {
                self.queue_wait_timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            _ => self.basic.set(k, v),
        }
    }
}

impl UserGroupConfig for LdapUserGroupConfig {
    fn basic_config(&self) -> &BasicUserGroupConfig {
        &self.basic
    }

    fn r#type(&self) -> &'static str {
        USER_GROUP_TYPE
    }
}
