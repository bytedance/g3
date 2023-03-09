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
use yaml_rust::{yaml, Yaml};

use g3_dpi::{MaybeProtocol, ProtocolPortMap};

fn as_maybe_protocol(value: &Yaml) -> anyhow::Result<Vec<MaybeProtocol>> {
    let mut r = Vec::new();

    match value {
        Yaml::String(s) => {
            let p = MaybeProtocol::from_str(s).map_err(|_| anyhow!("unrecognised protocol {s}"))?;
            r.push(p);
        }
        Yaml::Array(seq) => {
            for (i, v) in seq.iter().enumerate() {
                if let Yaml::String(s) = v {
                    let p = MaybeProtocol::from_str(s)
                        .map_err(|_| anyhow!("#{i}: unrecognised protocol {s}"))?;
                    r.push(p);
                } else {
                    return Err(anyhow!(
                        "the yaml value type for #{i} should be 'protocol string'"
                    ));
                }
            }
        }
        _ => return Err(anyhow!("invalid yaml value type")),
    }

    Ok(r)
}

fn update_by_map_value(portmap: &mut ProtocolPortMap, map: &yaml::Hash) -> anyhow::Result<()> {
    for (port, protocol) in map.iter() {
        let port = crate::value::as_u16(port)
            .context("the root map key should be valid u16 port value")?;
        let protocols = as_maybe_protocol(protocol)
            .context("the root map value should be valid protocol string(s) value")?;
        portmap.insert_batch(port, &protocols);
    }

    Ok(())
}

fn update_by_seq_value(portmap: &mut ProtocolPortMap, seq: &yaml::Array) -> anyhow::Result<()> {
    for (i, v) in seq.iter().enumerate() {
        if let Yaml::Hash(map) = v {
            let port = crate::hash_get_required(map, "port")?;
            let port =
                crate::value::as_u16(port).context("invalid u16 port value for key 'port'")?;
            let protocol = crate::hash_get_required(map, "protocol")?;
            let protocols = as_maybe_protocol(protocol)
                .context("invalid protocol string(s) value for key 'protocol'")?;
            portmap.insert_batch(port, &protocols);
        } else {
            return Err(anyhow!("the yaml value type for #{i} should be 'map'"));
        }
    }

    Ok(())
}

pub fn update_protocol_portmap(portmap: &mut ProtocolPortMap, value: &Yaml) -> anyhow::Result<()> {
    match value {
        Yaml::Hash(map) => update_by_map_value(portmap, map)
            .context("invalid yaml map value for 'protocol portmap'"),
        Yaml::Array(seq) => update_by_seq_value(portmap, seq)
            .context("invalid yaml seq value for 'protocol portmap'"),
        _ => Err(anyhow!("invalid yaml value type for 'protocol portmap'")),
    }
}
