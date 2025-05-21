/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::path::Path;
use std::str::FromStr;

use anyhow::{Context, anyhow};
use url::Url;
use yaml_rust::{Yaml, yaml};

use super::{IcapMethod, IcapServiceConfig};

impl IcapServiceConfig {
    fn parse_yaml(
        map: &yaml::Hash,
        method: IcapMethod,
        lookup_dir: Option<&Path>,
    ) -> anyhow::Result<Self> {
        const KEY_URL: &str = "url";
        let url = g3_yaml::hash_get_required(map, KEY_URL)?;
        let url = g3_yaml::value::as_url(url)
            .context(format!("invalid url string value for key {KEY_URL}"))?;
        let mut config = IcapServiceConfig::new(method, url)?;

        g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
            KEY_URL => Ok(()),
            "tls_client" => {
                let tls_client = g3_yaml::value::as_rustls_client_config_builder(v, lookup_dir)
                    .context(format!(
                        "invalid rustls tls client config value for key {k}"
                    ))?;
                config.set_tls_client(tls_client);
                Ok(())
            }
            "tls_name" => {
                let tls_name = g3_yaml::value::as_rustls_server_name(v)
                    .context(format!("invalid rustls server name value for key {k}"))?;
                config.set_tls_name(tls_name);
                Ok(())
            }
            "tcp_keepalive" => {
                let keepalive = g3_yaml::value::as_tcp_keepalive_config(v)
                    .context(format!("invalid tcp keepalive config value for key {k}"))?;
                config.set_tcp_keepalive(keepalive);
                Ok(())
            }
            "icap_connection_pool" | "connection_pool" | "pool" => {
                config.connection_pool = g3_yaml::value::as_connection_pool_config(v)
                    .context(format!("invalid connection pool config value for key {k}"))?;
                Ok(())
            }
            "icap_max_header_size" => {
                let size = g3_yaml::humanize::as_usize(v)
                    .context(format!("invalid humanize usize value for key {k}"))?;
                config.set_icap_max_header_size(size);
                Ok(())
            }
            "disable_preview" | "no_preview" => {
                config.disable_preview = g3_yaml::value::as_bool(v)?;
                Ok(())
            }
            "preview_data_read_timeout" => {
                let time = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                config.set_preview_data_read_timeout(time);
                Ok(())
            }
            "respond_shared_names" => {
                if let Yaml::Array(seq) = v {
                    for (i, v) in seq.iter().enumerate() {
                        let name = g3_yaml::value::as_http_header_name(v)
                            .context(format!("invalid http header name value for key {k}#{i}"))?;
                        config.add_respond_shared_name(name);
                    }
                } else {
                    let name = g3_yaml::value::as_http_header_name(v)
                        .context(format!("invalid http header name value for key {k}"))?;
                    config.add_respond_shared_name(name);
                }
                Ok(())
            }
            "bypass" => {
                let bypass = g3_yaml::value::as_bool(v)?;
                config.set_bypass(bypass);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        Ok(config)
    }

    pub fn parse_reqmod_service_yaml(
        value: &Yaml,
        lookup_dir: Option<&Path>,
    ) -> anyhow::Result<Self> {
        match value {
            Yaml::Hash(map) => Self::parse_yaml(map, IcapMethod::Reqmod, lookup_dir),
            Yaml::String(s) => {
                let url = Url::from_str(s).map_err(|e| anyhow!("invalid url string: {e}"))?;
                IcapServiceConfig::new(IcapMethod::Reqmod, url)
            }
            _ => Err(anyhow!(
                "yaml value type for 'icap service config' should be 'map' or 'url str'"
            )),
        }
    }

    pub fn parse_respmod_service_yaml(
        value: &Yaml,
        lookup_dir: Option<&Path>,
    ) -> anyhow::Result<Self> {
        match value {
            Yaml::Hash(map) => Self::parse_yaml(map, IcapMethod::Respmod, lookup_dir),
            Yaml::String(s) => {
                let url = Url::from_str(s).map_err(|e| anyhow!("invalid url string: {e}"))?;
                IcapServiceConfig::new(IcapMethod::Respmod, url)
            }
            _ => Err(anyhow!(
                "yaml value type for 'icap service config' should be 'map' or 'url str'"
            )),
        }
    }
}
