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
use serde_json::Value;

use g3_types::net::Ports;

fn as_single_ports(value: &Value) -> anyhow::Result<Ports> {
    match value {
        Value::Number(_) => {
            let port = crate::value::as_u16(value)?;
            let mut ports = Ports::default();
            ports.add_single(port);
            Ok(ports)
        }
        Value::String(s) => {
            let ports = Ports::from_str(s)?;
            Ok(ports)
        }
        _ => Err(anyhow!("invalid value type")),
    }
}

pub fn as_ports(value: &Value) -> anyhow::Result<Ports> {
    match value {
        Value::Number(_) | Value::String(_) => as_single_ports(value),
        Value::Array(seq) => {
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
