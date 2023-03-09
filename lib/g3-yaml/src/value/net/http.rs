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
use http::HeaderName;
use yaml_rust::Yaml;

use g3_types::net::{
    HttpForwardCapability, HttpForwardedHeaderType, HttpKeepAliveConfig, HttpServerId,
};

pub fn as_http_keepalive_config(v: &Yaml) -> anyhow::Result<HttpKeepAliveConfig> {
    let mut config = HttpKeepAliveConfig::default();

    match v {
        Yaml::Hash(map) => {
            crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
                "enable" => {
                    let enable = crate::value::as_bool(v)?;
                    config.set_enable(enable);
                    Ok(())
                }
                "idle_expire" => {
                    let idle_expire = crate::humanize::as_duration(v)?;
                    config.set_idle_expire(idle_expire);
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;
        }
        Yaml::Boolean(enable) => {
            config.set_enable(*enable);
        }
        _ => {
            let idle_expire = crate::humanize::as_duration(v)
                .context("invalid http keepalive idle_expire value")?;
            config.set_enable(true);
            config.set_idle_expire(idle_expire);
        }
    }

    Ok(config)
}

pub fn as_http_forwarded_header_type(value: &Yaml) -> anyhow::Result<HttpForwardedHeaderType> {
    match crate::value::as_bool(value) {
        Ok(true) => Ok(HttpForwardedHeaderType::default()),
        Ok(false) => Ok(HttpForwardedHeaderType::Disable),
        Err(_) => {
            if let Yaml::String(s) = value {
                HttpForwardedHeaderType::from_str(s)
                    .map_err(|_| anyhow!("invalid string value for 'HttpForwardedHeaderType'"))
            } else {
                Err(anyhow!(
                    "yaml value type for 'IpAddr' should be 'boolean' or 'string'"
                ))
            }
        }
    }
}

pub fn as_http_forward_capability(value: &Yaml) -> anyhow::Result<HttpForwardCapability> {
    let mut cap = HttpForwardCapability::default();

    match value {
        Yaml::Hash(map) => {
            if let Ok(v) = crate::hash_get_required(map, "forward_ftp") {
                let enable =
                    crate::value::as_bool(v).context("invalid bool value for key forward_ftp")?;
                cap.set_forward_ftp_all(enable);
            }

            crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
                "forward_https" => {
                    let enable = crate::value::as_bool(v)?;
                    cap.set_forward_https(enable);
                    Ok(())
                }
                "forward_ftp_get" => {
                    let enable = crate::value::as_bool(v)?;
                    cap.set_forward_ftp_get(enable);
                    Ok(())
                }
                "forward_ftp_put" => {
                    let enable = crate::value::as_bool(v)?;
                    cap.set_forward_ftp_put(enable);
                    Ok(())
                }
                "forward_ftp_del" => {
                    let enable = crate::value::as_bool(v)?;
                    cap.set_forward_ftp_del(enable);
                    Ok(())
                }
                "forward_ftp" => Ok(()),
                _ => Err(anyhow!("invalid key {k}")),
            })?;
        }
        _ => return Err(anyhow!("invalid yaml value type for HttpForwardCapability")),
    }

    Ok(cap)
}

pub fn as_http_server_id(value: &Yaml) -> anyhow::Result<HttpServerId> {
    if let Yaml::String(s) = value {
        let id = HttpServerId::from_str(s)?;
        Ok(id)
    } else {
        Err(anyhow!(
            "yaml value type for 'HttpServerId' should be 'string'"
        ))
    }
}

pub fn as_http_header_name(value: &Yaml) -> anyhow::Result<HeaderName> {
    if let Yaml::String(s) = value {
        let name = HeaderName::from_str(s)?;
        Ok(name)
    } else {
        Err(anyhow!(
            "yaml value type for 'HttpHeaderName' should be 'string'"
        ))
    }
}
