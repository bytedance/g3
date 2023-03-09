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
use yaml_rust::Yaml;

use g3_types::net::UdpMiscSockOpts;

pub fn as_udp_misc_sock_opts(v: &Yaml) -> anyhow::Result<UdpMiscSockOpts> {
    let mut config = UdpMiscSockOpts::default();

    if let Yaml::Hash(map) = v {
        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "time_to_live" | "ttl" => {
                let ttl =
                    crate::value::as_u32(v).context(format!("invalid u32 value for key {k}"))?;
                config.time_to_live = Some(ttl);
                Ok(())
            }
            "type_of_service" | "tos" => {
                let tos =
                    crate::value::as_u8(v).context(format!("invalid u8 value for key {k}"))?;
                config.type_of_service = Some(tos);
                Ok(())
            }
            "netfilter_mark" | "mark" => {
                let mark =
                    crate::value::as_u32(v).context(format!("invalid u32 value for key {k}"))?;
                config.netfilter_mark = Some(mark);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        Ok(config)
    } else {
        Err(anyhow!(
            "yaml value type for 'UdpMiscSockOpts' should be 'map'"
        ))
    }
}
