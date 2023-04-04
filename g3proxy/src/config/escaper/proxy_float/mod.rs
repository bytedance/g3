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

use std::collections::BTreeSet;
use std::net::IpAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context};
use ascii::AsciiString;
use log::warn;
use yaml_rust::{yaml, Yaml};

use g3_types::metrics::{MetricsName, StaticMetricsTags};
use g3_types::net::{OpensslTlsClientConfigBuilder, TcpKeepAliveConfig, TcpMiscSockOpts};
use g3_yaml::YamlDocPosition;

use super::{AnyEscaperConfig, EscaperConfig, EscaperConfigDiffAction};

pub(crate) mod source;
pub(crate) use source::ProxyFloatSource;

const ESCAPER_CONFIG_TYPE: &str = "ProxyFloat";

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct ProxyFloatEscaperConfig {
    pub(crate) name: MetricsName,
    position: Option<YamlDocPosition>,
    pub(crate) shared_logger: Option<AsciiString>,
    pub(crate) bind_v4: Option<IpAddr>,
    pub(crate) bind_v6: Option<IpAddr>,
    pub(crate) tls_config: Option<OpensslTlsClientConfigBuilder>,
    pub(crate) source: Option<ProxyFloatSource>,
    pub(crate) cache_file: Option<PathBuf>,
    pub(crate) refresh_interval: Duration,
    pub(crate) tcp_connect_timeout: Duration,
    pub(crate) tcp_keepalive: TcpKeepAliveConfig,
    pub(crate) tcp_misc_opts: TcpMiscSockOpts,
    pub(crate) expire_guard_duration: chrono::Duration,
    pub(crate) peer_negotiation_timeout: Duration,
    pub(crate) extra_metrics_tags: Option<Arc<StaticMetricsTags>>,
}

impl ProxyFloatEscaperConfig {
    fn new(position: Option<YamlDocPosition>) -> Self {
        ProxyFloatEscaperConfig {
            name: MetricsName::default(),
            position,
            shared_logger: None,
            bind_v4: None,
            bind_v6: None,
            tls_config: None,
            source: None,
            cache_file: None,
            refresh_interval: Duration::from_secs(1),
            tcp_connect_timeout: Duration::from_secs(30),
            tcp_keepalive: TcpKeepAliveConfig::default_enabled(),
            tcp_misc_opts: Default::default(),
            expire_guard_duration: chrono::Duration::seconds(5),
            peer_negotiation_timeout: Duration::from_secs(10),
            extra_metrics_tags: None,
        }
    }

    pub(super) fn parse(
        map: &yaml::Hash,
        position: Option<YamlDocPosition>,
    ) -> anyhow::Result<Self> {
        let mut config = Self::new(position);

        g3_yaml::foreach_kv(map, |k, v| config.set(k, v))?;

        config.check()?;
        Ok(config)
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            super::CONFIG_KEY_ESCAPER_TYPE => Ok(()),
            super::CONFIG_KEY_ESCAPER_NAME => {
                self.name = g3_yaml::value::as_metrics_name(v)?;
                Ok(())
            }
            "shared_logger" => {
                let name = g3_yaml::value::as_ascii(v)?;
                self.shared_logger = Some(name);
                Ok(())
            }
            "extra_metrics_tags" => {
                let tags = g3_yaml::value::as_static_metrics_tags(v)
                    .context(format!("invalid static metrics tags value for key {k}"))?;
                self.extra_metrics_tags = Some(Arc::new(tags));
                Ok(())
            }
            "bind_ipv4" => {
                let ip4 = g3_yaml::value::as_ipv4addr(v)?;
                self.bind_v4 = Some(IpAddr::V4(ip4));
                Ok(())
            }
            "bind_ipv6" => {
                let ip6 = g3_yaml::value::as_ipv6addr(v)?;
                self.bind_v6 = Some(IpAddr::V6(ip6));
                Ok(())
            }
            "tls" | "tls_client" => {
                if let Yaml::Boolean(enable) = v {
                    if *enable {
                        self.tls_config =
                            Some(OpensslTlsClientConfigBuilder::with_cache_for_many_sites());
                    }
                } else {
                    let lookup_dir = crate::config::get_lookup_dir(self.position.as_ref());
                    let builder = g3_yaml::value::as_to_many_openssl_tls_client_config_builder(
                        v,
                        Some(&lookup_dir),
                    )
                    .context(format!(
                        "invalid openssl tls client config value for key {k}"
                    ))?;
                    self.tls_config = Some(builder);
                }
                Ok(())
            }
            "source" => {
                self.source =
                    Some(ProxyFloatSource::parse(v).context(format!("invalid value for key {k}"))?);
                Ok(())
            }
            "cache" => {
                let lookup_dir = crate::config::get_lookup_dir(self.position.as_ref());
                self.cache_file = Some(
                    g3_yaml::value::as_file_path(v, &lookup_dir, true)
                        .context(format!("invalid value for key {k}"))?,
                );
                Ok(())
            }
            "refresh_interval" => {
                self.refresh_interval = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid duration value for key {k}"))?;
                Ok(())
            }
            "tcp_connect_timeout" => {
                self.tcp_connect_timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "tcp_keepalive" => {
                self.tcp_keepalive = g3_yaml::value::as_tcp_keepalive_config(v)
                    .context(format!("invalid tcp keepalive config for key {k}"))?;
                Ok(())
            }
            "tcp_misc_opts" => {
                self.tcp_misc_opts = g3_yaml::value::as_tcp_misc_sock_opts(v)
                    .context(format!("invalid tcp misc sock opts value for key {k}"))?;
                Ok(())
            }
            "expire_guard_duration" => {
                let dur = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                self.expire_guard_duration = chrono::Duration::from_std(dur)
                    .map_err(|e| anyhow!("invalid duration: {e}"))?;
                Ok(())
            }
            "peer_negotiation_timeout" => {
                self.peer_negotiation_timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }

    fn check(&mut self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }
        if let Some(source) = &self.source {
            if source.need_local_cache() && self.cache_file.is_none() {
                warn!(
                    "It is very recommended to set local cache for escaper {}",
                    self.name
                );
            }
        } else {
            return Err(anyhow!("no source url set"));
        }

        Ok(())
    }
}

impl EscaperConfig for ProxyFloatEscaperConfig {
    fn name(&self) -> &MetricsName {
        &self.name
    }

    fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn escaper_type(&self) -> &str {
        ESCAPER_CONFIG_TYPE
    }

    fn resolver(&self) -> &MetricsName {
        Default::default()
    }

    fn shared_logger(&self) -> Option<&str> {
        self.shared_logger.as_ref().map(|s| s.as_str())
    }

    fn diff_action(&self, new: &AnyEscaperConfig) -> EscaperConfigDiffAction {
        let new = match new {
            AnyEscaperConfig::ProxyFloat(config) => config,
            _ => return EscaperConfigDiffAction::SpawnNew,
        };

        if self.eq(new) {
            return EscaperConfigDiffAction::NoAction;
        }

        EscaperConfigDiffAction::Reload
    }

    fn dependent_escaper(&self) -> Option<BTreeSet<MetricsName>> {
        None
    }
}
