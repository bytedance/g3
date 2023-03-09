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

use anyhow::{anyhow, Context};
use url::Url;
use yaml_rust::{yaml, Yaml};

use g3_icap_client::{IcapConnectionPoolConfig, IcapMethod, IcapServiceConfig};

fn set_icap_connection_pool_config(
    config: &mut IcapConnectionPoolConfig,
    value: &Yaml,
) -> anyhow::Result<()> {
    if let Yaml::Hash(map) = value {
        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "check_interval" => {
                let interval = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                config.set_check_interval(interval);
                Ok(())
            }
            "max_idle_count" => {
                let count = crate::value::as_usize(v)?;
                config.set_max_idle_count(count);
                Ok(())
            }
            "min_idle_count" => {
                let count = crate::value::as_usize(v)?;
                config.set_min_idle_count(count);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })
    } else {
        Err(anyhow!(
            "yaml value type for 'icap connection pool' should be 'map'"
        ))
    }
}

fn as_icap_service_config(
    map: &yaml::Hash,
    method: IcapMethod,
) -> anyhow::Result<IcapServiceConfig> {
    const KEY_URL: &str = "url";
    let url = crate::hash_get_required(map, KEY_URL)?;
    let url =
        crate::value::as_url(url).context(format!("invalid url string value for key {KEY_URL}"))?;
    let mut config = IcapServiceConfig::new(method, url)?;

    crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
        KEY_URL => Ok(()),
        "tcp_keepalive" => {
            let keepalive = crate::value::as_tcp_keepalive_config(v)
                .context(format!("invalid tcp keepalive config value for key {k}"))?;
            config.set_tcp_keepalive(keepalive);
            Ok(())
        }
        "icap_connection_pool" | "connection_pool" | "pool" => {
            set_icap_connection_pool_config(&mut config.connection_pool, v).context(format!(
                "invalid icap connection pool config value for key {k}"
            ))
        }
        "icap_max_header_size" => {
            let size = crate::humanize::as_usize(v)
                .context(format!("invalid humanize usize value for key {k}"))?;
            config.set_icap_max_header_size(size);
            Ok(())
        }
        "preview_data_read_timeout" => {
            let time = crate::humanize::as_duration(v)
                .context(format!("invalid humanize duration value for key {k}"))?;
            config.set_preview_data_read_timeout(time);
            Ok(())
        }
        "respond_shared_names" => {
            if let Yaml::Array(seq) = v {
                for (i, v) in seq.iter().enumerate() {
                    let name = crate::value::as_http_header_name(v)
                        .context(format!("invalid http header name value for key {k}#{i}"))?;
                    config.add_respond_shared_name(name);
                }
            } else {
                let name = crate::value::as_http_header_name(v)
                    .context(format!("invalid http header name value for key {k}"))?;
                config.add_respond_shared_name(name);
            }
            Ok(())
        }
        "bypass" => {
            let bypass = crate::value::as_bool(v)?;
            config.set_bypass(bypass);
            Ok(())
        }
        _ => Err(anyhow!("invalid key {k}")),
    })?;

    Ok(config)
}

pub fn as_icap_reqmod_service_config(value: &Yaml) -> anyhow::Result<IcapServiceConfig> {
    match value {
        Yaml::Hash(map) => as_icap_service_config(map, IcapMethod::Reqmod),
        Yaml::String(s) => {
            let url = Url::from_str(s).map_err(|e| anyhow!("invalid url string: {e}"))?;
            IcapServiceConfig::new(IcapMethod::Reqmod, url)
        }
        _ => Err(anyhow!(
            "yaml value type for 'icap service config' should be 'map' or 'url str'"
        )),
    }
}

pub fn as_icap_respmod_service_config(value: &Yaml) -> anyhow::Result<IcapServiceConfig> {
    match value {
        Yaml::Hash(map) => as_icap_service_config(map, IcapMethod::Respmod),
        Yaml::String(s) => {
            let url = Url::from_str(s).map_err(|e| anyhow!("invalid url string: {e}"))?;
            IcapServiceConfig::new(IcapMethod::Respmod, url)
        }
        _ => Err(anyhow!(
            "yaml value type for 'icap service config' should be 'map' or 'url str'"
        )),
    }
}
