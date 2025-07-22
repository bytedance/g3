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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn as_ports_ok() {
        // valid number input (single port)
        let value = json!(8080);
        let ports = as_ports(&value).unwrap();
        assert!(ports.contains(8080));

        // valid string input (single port)
        let value = json!("80");
        let ports = as_ports(&value).unwrap();
        assert!(ports.contains(80));

        // valid string input (port range)
        let value = json!("1000-1002");
        let ports = as_ports(&value).unwrap();
        assert!(ports.contains(1000));
        assert!(ports.contains(1001));
        assert!(ports.contains(1002));

        // valid string input (mixed expression)
        let value = json!("80,443,3000-3002");
        let ports = as_ports(&value).unwrap();
        assert!(ports.contains(80));
        assert!(ports.contains(443));
        assert!(ports.contains(3000));
        assert!(ports.contains(3001));
        assert!(ports.contains(3002));

        // valid array input (pure numbers)
        let value = json!([8080, 8081]);
        let ports = as_ports(&value).unwrap();
        assert!(ports.contains(8080));
        assert!(ports.contains(8081));

        // valid array input (pure strings)
        let value = json!(["80-82", "443"]);
        let ports = as_ports(&value).unwrap();
        assert!(ports.contains(80));
        assert!(ports.contains(81));
        assert!(ports.contains(82));
        assert!(ports.contains(443));

        // valid array input (mixed types)
        let value = json!([8080, "443", "3000-3002"]);
        let ports = as_ports(&value).unwrap();
        assert!(ports.contains(8080));
        assert!(ports.contains(443));
        assert!(ports.contains(3000));
        assert!(ports.contains(3001));
        assert!(ports.contains(3002));

        // boundary values
        let value = json!([0, 65535]);
        let ports = as_ports(&value).unwrap();
        assert!(ports.contains(0));
        assert!(ports.contains(65535));
    }

    #[test]
    fn as_ports_err() {
        // invalid type
        let value = json!(true);
        assert!(as_ports(&value).is_err());

        let value = json!({ "key": "value" });
        assert!(as_ports(&value).is_err());

        // invalid number (out of u16 range)
        let value = json!(70000);
        assert!(as_ports(&value).is_err());

        // invalid string format
        let value = json!("1000-2000-3000");
        assert!(as_ports(&value).is_err());

        let value = json!("abc");
        assert!(as_ports(&value).is_err());

        // invalid range (start > end)
        let value = json!("2000-1000");
        assert!(as_ports(&value).is_err());

        // array with invalid element
        let value = json!([8080, true, "443"]);
        assert!(as_ports(&value).is_err());
    }
}
