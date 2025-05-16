/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::{Context, anyhow};
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
