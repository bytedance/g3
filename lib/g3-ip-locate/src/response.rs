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

use std::net::IpAddr;

use anyhow::{Context, anyhow};
use rmpv::ValueRef;

use g3_geoip_types::{IpLocation, IpLocationBuilder};

use super::{response_key, response_key_id};

#[derive(Default)]
pub struct Response {
    ip: Option<IpAddr>,
    location_builder: IpLocationBuilder,
    ttl: Option<u32>,
}

impl Response {
    fn set(&mut self, k: ValueRef, v: ValueRef) -> anyhow::Result<()> {
        match k {
            ValueRef::String(s) => {
                let key = s
                    .as_str()
                    .ok_or_else(|| anyhow!("invalid string key {k}"))?;
                match g3_msgpack::key::normalize(key).as_str() {
                    response_key::IP => {
                        let ip = g3_msgpack::value::as_ipaddr(&v)
                            .context(format!("invalid ip address value for key {key}"))?;
                        self.ip = Some(ip);
                    }
                    response_key::TTL => {
                        let ttl = g3_msgpack::value::as_u32(&v)
                            .context(format!("invalid u32 value for key {key}"))?;
                        self.ttl = Some(ttl);
                    }
                    response_key::NETWORK => {
                        let network = g3_msgpack::value::as_ip_network(&v)
                            .context(format!("invalid ip network value for key {key}"))?;
                        self.location_builder.set_network(network);
                    }
                    response_key::COUNTRY => {
                        let country = g3_msgpack::value::as_iso_country_code(&v)
                            .context(format!("invalid iso country code value for key {key}"))?;
                        self.location_builder.set_country(country);
                    }
                    response_key::CONTINENT => {
                        let continent = g3_msgpack::value::as_continent_code(&v)
                            .context(format!("invalid continent code value for key {key}"))?;
                        self.location_builder.set_continent(continent);
                    }
                    response_key::AS_NUMBER => {
                        let number = g3_msgpack::value::as_u32(&v)
                            .context(format!("invalid u32 value for key {key}"))?;
                        self.location_builder.set_as_number(number);
                    }
                    response_key::ISP_NAME => {
                        let name = g3_msgpack::value::as_string(&v)
                            .context(format!("invalid string value for key {key}"))?;
                        self.location_builder.set_isp_name(name);
                    }
                    response_key::ISP_DOMAIN => {
                        let domain = g3_msgpack::value::as_string(&v)
                            .context(format!("invalid string value for key {key}"))?;
                        self.location_builder.set_isp_domain(domain);
                    }
                    _ => {} // ignore unknown keys
                }
            }
            ValueRef::Integer(i) => {
                let key_id = i.as_u64().ok_or_else(|| anyhow!("invalid u64 key {k}"))?;
                match key_id {
                    response_key_id::IP => {
                        let ip = g3_msgpack::value::as_ipaddr(&v)
                            .context(format!("invalid ip address value for key id {key_id}"))?;
                        self.ip = Some(ip);
                    }
                    response_key_id::TTL => {
                        let ttl = g3_msgpack::value::as_u32(&v)
                            .context(format!("invalid u32 value for key id {key_id}"))?;
                        self.ttl = Some(ttl);
                    }
                    response_key_id::NETWORK => {
                        let network = g3_msgpack::value::as_ip_network(&v)
                            .context(format!("invalid ip network value for key id {key_id}"))?;
                        self.location_builder.set_network(network);
                    }
                    response_key_id::COUNTRY => {
                        let country = g3_msgpack::value::as_iso_country_code(&v).context(
                            format!("invalid iso country code value for key id {key_id}"),
                        )?;
                        self.location_builder.set_country(country);
                    }
                    response_key_id::CONTINENT => {
                        let continent = g3_msgpack::value::as_continent_code(&v)
                            .context(format!("invalid continent code value for key id {key_id}"))?;
                        self.location_builder.set_continent(continent);
                    }
                    response_key_id::AS_NUMBER => {
                        let number = g3_msgpack::value::as_u32(&v)
                            .context(format!("invalid u32 value for key id {key_id}"))?;
                        self.location_builder.set_as_number(number);
                    }
                    response_key_id::ISP_NAME => {
                        let name = g3_msgpack::value::as_string(&v)
                            .context(format!("invalid string value for key id {key_id}"))?;
                        self.location_builder.set_isp_name(name);
                    }
                    response_key_id::ISP_DOMAIN => {
                        let domain = g3_msgpack::value::as_string(&v)
                            .context(format!("invalid string value for key id {key_id}"))?;
                        self.location_builder.set_isp_domain(domain);
                    }
                    _ => {} // ignore unknown keys
                }
            }
            _ => return Err(anyhow!("unsupported key type: {k}")),
        }
        Ok(())
    }

    pub(super) fn parse(v: ValueRef) -> anyhow::Result<Self> {
        if let ValueRef::Map(map) = v {
            let mut response = Response::default();
            for (k, v) in map {
                response.set(k, v)?;
            }
            Ok(response)
        } else {
            Err(anyhow!("the response data type should be 'map'"))
        }
    }

    pub(super) fn into_parts(self) -> (Option<IpAddr>, Option<IpLocation>, Option<u32>) {
        let location = self.location_builder.build().ok();
        (self.ip, location, self.ttl)
    }

    pub fn encode_new(ip: IpAddr, location: IpLocation, ttl: u32) -> anyhow::Result<Vec<u8>> {
        let ip = ip.to_string();
        let network = location.network_addr().to_string();
        let mut map = vec![
            (
                ValueRef::Integer(response_key_id::IP.into()),
                ValueRef::String(ip.as_str().into()),
            ),
            (
                ValueRef::Integer(response_key_id::NETWORK.into()),
                ValueRef::String(network.as_str().into()),
            ),
            (
                ValueRef::Integer(response_key_id::TTL.into()),
                ValueRef::Integer(ttl.into()),
            ),
        ];
        if let Some(country) = location.country() {
            map.push((
                ValueRef::Integer(response_key_id::COUNTRY.into()),
                ValueRef::String(country.alpha2_code().into()),
            ));
        }
        if let Some(continent) = location.continent() {
            map.push((
                ValueRef::Integer(response_key_id::CONTINENT.into()),
                ValueRef::String(continent.code().into()),
            ));
        }
        if let Some(number) = location.network_asn() {
            map.push((
                ValueRef::Integer(response_key_id::AS_NUMBER.into()),
                ValueRef::Integer(number.into()),
            ));
        }
        if let Some(name) = location.isp_name() {
            map.push((
                ValueRef::Integer(response_key_id::ISP_NAME.into()),
                ValueRef::String(name.into()),
            ));
        }
        if let Some(domain) = location.isp_domain() {
            map.push((
                ValueRef::Integer(response_key_id::ISP_DOMAIN.into()),
                ValueRef::String(domain.into()),
            ));
        }
        let mut buf = Vec::with_capacity(4096);
        let v = ValueRef::Map(map);
        rmpv::encode::write_value_ref(&mut buf, &v)
            .map_err(|e| anyhow!("msgpack encode failed: {e}"))?;
        Ok(buf)
    }
}
