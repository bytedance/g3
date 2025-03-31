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

use std::str::FromStr;

use g3_types::net::Interface;

use anyhow::anyhow;
use yaml_rust::Yaml;

pub fn as_interface(value: &Yaml) -> anyhow::Result<Interface> {
    match value {
        Yaml::String(s) => {
            Interface::from_str(s).map_err(|e| anyhow!("invalid interface name {s}: {e}"))
        }
        Yaml::Integer(i) => {
            let u = u32::try_from(*i).map_err(|_| anyhow!("out of range u32 value {}", *i))?;
            Interface::try_from(u).map_err(|e| anyhow!("invalid interface id {u}: {e}"))
        }
        _ => Err(anyhow!(
            "yaml value type for 'InterfaceName' should be 'string' or 'u32'"
        )),
    }
}
