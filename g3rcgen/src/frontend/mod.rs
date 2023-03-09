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

use anyhow::{anyhow, Context};
use rmpv::ValueRef;

mod udp_dgram;
pub(crate) use udp_dgram::UdpDgramFrontend;

#[derive(Debug)]
pub(crate) struct ResponseData {
    pub(crate) host: String,
    pub(crate) cert: String,
    pub(crate) key: String,
    pub(crate) ttl: u32,
}

impl ResponseData {
    pub(crate) fn encode(&self) -> anyhow::Result<Vec<u8>> {
        let map = vec![
            (
                ValueRef::String("host".into()),
                ValueRef::String(self.host.as_str().into()),
            ),
            (
                ValueRef::String("cert".into()),
                ValueRef::String(self.cert.as_str().into()),
            ),
            (
                ValueRef::String("key".into()),
                ValueRef::String(self.key.as_str().into()),
            ),
            (
                ValueRef::String("ttl".into()),
                ValueRef::Integer(self.ttl.into()),
            ),
        ];
        let mut buf = Vec::with_capacity(32);
        let v = ValueRef::Map(map);
        rmpv::encode::write_value_ref(&mut buf, &v)
            .map_err(|e| anyhow!("msgpack encode failed: {e}"))?;
        Ok(buf)
    }
}

pub(crate) fn decode_req(mut data: &[u8]) -> anyhow::Result<String> {
    let v =
        rmpv::decode::read_value_ref(&mut data).map_err(|e| anyhow!("invalid req data: {e}"))?;

    if let ValueRef::Map(map) = v {
        let mut host = String::default();

        for (k, v) in map {
            let key = g3_msgpack::value::as_string(&k)?;
            match g3_msgpack::key::normalize(key.as_str()).as_str() {
                "host" => {
                    host = g3_msgpack::value::as_string(&v)
                        .context(format!("invalid string value for key {key}"))?;
                }
                _ => return Err(anyhow!("invalid key {key}")),
            }
        }

        if host.is_empty() {
            Err(anyhow!("invalid host value"))
        } else {
            Ok(host)
        }
    } else {
        Err(anyhow!("the req root data type should be map"))
    }
}
