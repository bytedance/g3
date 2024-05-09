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

use anyhow::{anyhow, Context};
use rmpv::ValueRef;

use g3_geoip::IpLocation;

use super::{response_key, response_key_id};

#[derive(Default)]
pub(super) struct Response {
    ip: Option<IpAddr>,
    location: Option<IpLocation>,
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
                    response_key::LOCATION => {
                        let location = g3_msgpack::value::as_ip_location(&v)
                            .context(format!("invalid ip location value for key {key}"))?;
                        self.location = Some(location);
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
                    response_key_id::LOCATION => {
                        let location = g3_msgpack::value::as_ip_location(&v)
                            .context(format!("invalid ip location value for key id {key_id}"))?;
                        self.location = Some(location);
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
        (self.ip, self.location, self.ttl)
    }
}
