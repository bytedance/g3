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
use yaml_rust::Yaml;

use g3_geoip::{ContinentCode, IpLocation, IpLocationBuilder, IsoCountryCode};
use g3_ip_locate::IpLocateServiceConfig;

pub fn as_iso_country_code(value: &Yaml) -> anyhow::Result<IsoCountryCode> {
    if let Yaml::String(s) = value {
        let country =
            IsoCountryCode::from_str(s).map_err(|_| anyhow!("invalid iso country code"))?;
        Ok(country)
    } else {
        Err(anyhow!(
            "yaml value type for 'iso country code' should be 'string'"
        ))
    }
}

pub fn as_continent_code(value: &Yaml) -> anyhow::Result<ContinentCode> {
    if let Yaml::String(s) = value {
        let country = ContinentCode::from_str(s).map_err(|_| anyhow!("invalid continent code"))?;
        Ok(country)
    } else {
        Err(anyhow!(
            "yaml value type for 'continent code' should be 'string'"
        ))
    }
}

pub fn as_ip_location(value: &Yaml) -> anyhow::Result<IpLocation> {
    if let Yaml::Hash(map) = value {
        let mut builder = IpLocationBuilder::default();

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "network" | "net" => {
                let net = crate::value::as_ip_network(v)
                    .context(format!("invalid ip network value for key {k}"))?;
                builder.set_network(net);
                Ok(())
            }
            "country" => {
                let country = as_iso_country_code(v)
                    .context(format!("invalid iso country code value for key {k}"))?;
                builder.set_country(country);
                Ok(())
            }
            "continent" => {
                let continent = as_continent_code(v)
                    .context(format!("invalid continent code value for key {k}"))?;
                builder.set_continent(continent);
                Ok(())
            }
            "as_number" | "asn" => {
                let asn =
                    crate::value::as_u32(v).context(format!("invalid u32 value for key {k}"))?;
                builder.set_as_number(asn);
                Ok(())
            }
            "isp_name" => {
                let name = crate::value::as_string(v)
                    .context(format!("invalid string value for key {k}"))?;
                builder.set_isp_name(name);
                Ok(())
            }
            "isp_domain" => {
                let domain = crate::value::as_string(v)
                    .context(format!("invalid string value for key {k}"))?;
                builder.set_isp_domain(domain);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        builder.build()
    } else {
        Err(anyhow!("yaml value type for 'ip location' should be 'map'"))
    }
}

fn set_query_peer_addr(config: &mut IpLocateServiceConfig, value: &Yaml) -> anyhow::Result<()> {
    let addr = crate::value::as_env_sockaddr(value)?;
    config.set_query_peer_addr(addr);
    Ok(())
}

pub fn as_ip_locate_service_config(value: &Yaml) -> anyhow::Result<IpLocateServiceConfig> {
    match value {
        Yaml::Hash(map) => {
            let mut config = IpLocateServiceConfig::default();

            crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
                "cache_request_batch_count" => {
                    let count = crate::value::as_usize(v)?;
                    config.set_cache_request_batch_count(count);
                    Ok(())
                }
                "cache_request_timeout" => {
                    let time = crate::humanize::as_duration(v)
                        .context(format!("invalid humanize duration value for key {k}"))?;
                    config.set_cache_request_timeout(time);
                    Ok(())
                }
                "query_peer_addr" => {
                    set_query_peer_addr(&mut config, v)
                        .context(format!("invalid sockaddr str value for key {k}"))?;
                    Ok(())
                }
                "query_socket_buffer" => {
                    let buf_config = crate::value::as_socket_buffer_config(v)
                        .context(format!("invalid socket buffer config value for key {k}"))?;
                    config.set_query_socket_buffer(buf_config);
                    Ok(())
                }
                "query_wait_timeout" => {
                    let time = crate::humanize::as_duration(v)
                        .context(format!("invalid humanize duration value for key {k}"))?;
                    config.set_query_wait_timeout(time);
                    Ok(())
                }
                "default_expire_ttl" => {
                    let ttl = crate::value::as_u32(v)?;
                    config.set_default_expire_ttl(ttl);
                    Ok(())
                }
                "maximum_expire_ttl" => {
                    let ttl = crate::value::as_u32(v)?;
                    config.set_maximum_expire_ttl(ttl);
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;

            Ok(config)
        }
        Yaml::String(_) => {
            let mut config = IpLocateServiceConfig::default();
            set_query_peer_addr(&mut config, value).context("invalid sockaddr str value")?;
            Ok(config)
        }
        _ => Err(anyhow!(
            "yaml type for 'ip location service config' should be 'map'"
        )),
    }
}
