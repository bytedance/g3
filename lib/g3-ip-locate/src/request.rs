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

use super::{request_key, request_key_id};

#[derive(Default)]
pub struct Request {
    ip: Option<IpAddr>,
}

impl Request {
    fn set(&mut self, k: ValueRef, v: ValueRef) -> anyhow::Result<()> {
        match k {
            ValueRef::String(s) => {
                let key = s
                    .as_str()
                    .ok_or_else(|| anyhow!("invalid string key {k}"))?;
                match g3_msgpack::key::normalize(key).as_str() {
                    request_key::IP => self
                        .set_ip_value(v)
                        .context(format!("invalid ip address value for key {key}")),
                    _ => Err(anyhow!("invalid key {key}")),
                }
            }
            ValueRef::Integer(i) => {
                let key_id = i.as_u64().ok_or_else(|| anyhow!("invalid u64 key {k}"))?;
                match key_id {
                    request_key_id::IP => self
                        .set_ip_value(v)
                        .context(format!("invalid ip address value for key id {key_id}")),
                    _ => Err(anyhow!("invalid key id {key_id}")),
                }
            }
            _ => Err(anyhow!("unsupported key type: {k}")),
        }
    }

    fn set_ip_value(&mut self, v: ValueRef) -> anyhow::Result<()> {
        let ip = g3_msgpack::value::as_ipaddr(&v)?;
        self.ip = Some(ip);
        Ok(())
    }

    #[inline]
    pub fn ip(&self) -> Option<IpAddr> {
        self.ip
    }

    pub fn parse_req(mut data: &[u8]) -> anyhow::Result<Self> {
        let v = rmpv::decode::read_value_ref(&mut data)
            .map_err(|e| anyhow!("invalid req data: {e}"))?;

        let mut request = Request::default();
        if let ValueRef::Map(map) = v {
            for (k, v) in map {
                request.set(k, v)?;
            }
        } else {
            request
                .set_ip_value(v)
                .context("invalid single host string value")?;
        }

        Ok(request)
    }

    pub fn encode_new(ip: IpAddr) -> Result<Vec<u8>, ()> {
        let ip = ip.to_string();
        let value = ValueRef::String(ip.as_str().into());

        let mut buf = Vec::with_capacity(320);
        rmpv::encode::write_value_ref(&mut buf, &value).map_err(|_| ())?;
        Ok(buf)
    }
}
