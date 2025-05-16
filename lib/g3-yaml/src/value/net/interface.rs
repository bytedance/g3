/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
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
