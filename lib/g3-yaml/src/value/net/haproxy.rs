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

use yaml_rust::Yaml;

use anyhow::{anyhow, Context};
use g3_types::net::ProxyProtocolVersion;

pub fn as_proxy_protocol_version(value: &Yaml) -> anyhow::Result<ProxyProtocolVersion> {
    let v =
        crate::value::as_u8(value).context("ProxyProtocolVersion should be a valid u8 value")?;
    match v {
        1 => Ok(ProxyProtocolVersion::V1),
        2 => Ok(ProxyProtocolVersion::V2),
        _ => Err(anyhow!("unsupported PROXY protocol version {v}")),
    }
}
