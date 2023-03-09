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

use std::convert::TryFrom;
use std::str::FromStr;

use anyhow::{anyhow, Context};
use yaml_rust::Yaml;

use g3_types::net::{PortRange, Ports};

fn as_single_ports(value: &Yaml) -> anyhow::Result<Ports> {
    match value {
        Yaml::Integer(i) => {
            let port = u16::try_from(*i).map_err(|e| anyhow!("invalid u16 string: {e}"))?;
            let mut ports = Ports::default();
            ports.add_single(port);
            Ok(ports)
        }
        Yaml::String(s) => {
            let ports = Ports::from_str(s)?;
            Ok(ports)
        }
        _ => Err(anyhow!("invalid value type")),
    }
}

pub fn as_ports(value: &Yaml) -> anyhow::Result<Ports> {
    match value {
        Yaml::Integer(_) | Yaml::String(_) => as_single_ports(value),
        Yaml::Array(seq) => {
            let mut ports = Ports::default();

            for (i, v) in seq.iter().enumerate() {
                let p = as_single_ports(v).context(format!("invalid value for element #{i}"))?;
                ports.extend(p);
            }

            Ok(ports)
        }
        _ => Err(anyhow!("invalid value type")),
    }
}

pub fn as_port_range(value: &Yaml) -> anyhow::Result<PortRange> {
    match value {
        Yaml::String(s) => PortRange::from_str(s),
        Yaml::Hash(map) => {
            let mut start = 0;
            let mut end = 0;

            crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
                "start" | "from" => {
                    start = crate::value::as_u16(v)
                        .context(format!("invalid port number for key {k}"))?;
                    Ok(())
                }
                "end" | "to" => {
                    end = crate::value::as_u16(v)
                        .context(format!("invalid port number for key {k}"))?;
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })
            .context("invalid port range map value")?;

            let range = PortRange::new(start, end);
            range.check()?;
            Ok(range)
        }
        _ => Err(anyhow!("invalid value type")),
    }
}
