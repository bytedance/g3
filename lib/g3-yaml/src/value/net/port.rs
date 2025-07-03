/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::{Context, anyhow};
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_single_ports_ok() {
        // Valid integer
        let yaml = Yaml::Integer(8080);
        let ports = as_single_ports(&yaml).unwrap();
        assert!(ports.contains(8080));
        assert!(!ports.contains(80));

        // Valid string - single port
        let yaml = yaml_str!("80");
        let ports = as_single_ports(&yaml).unwrap();
        assert!(ports.contains(80));
        assert!(!ports.contains(443));

        // Valid string - multiple ports
        let yaml = yaml_str!("80,443");
        let ports = as_single_ports(&yaml).unwrap();
        assert!(ports.contains(80));
        assert!(ports.contains(443));

        // Valid string - range
        let yaml = yaml_str!("8000-8010");
        let ports = as_single_ports(&yaml).unwrap();
        assert!(ports.contains(8000));
        assert!(ports.contains(8005));
        assert!(ports.contains(8010));
    }

    #[test]
    fn as_single_ports_err() {
        // Invalid integer (out of range)
        let yaml = Yaml::Integer(65536);
        assert!(as_single_ports(&yaml).is_err());

        // Invalid string
        let yaml = yaml_str!("invalid");
        assert!(as_single_ports(&yaml).is_err());

        // Invalid type
        let yaml = Yaml::Boolean(true);
        assert!(as_single_ports(&yaml).is_err());
    }

    #[test]
    fn as_ports_ok() {
        // Single integer
        let yaml = Yaml::Integer(80);
        let ports = as_ports(&yaml).unwrap();
        assert!(ports.contains(80));

        // Single string
        let yaml = yaml_str!("443");
        let ports = as_ports(&yaml).unwrap();
        assert!(ports.contains(443));

        // Array of integers and strings
        let yaml = Yaml::Array(vec![
            Yaml::Integer(80),
            yaml_str!("443"),
            yaml_str!("8000-8010"),
        ]);
        let ports = as_ports(&yaml).unwrap();
        assert!(ports.contains(80));
        assert!(ports.contains(443));
        assert!(ports.contains(8000));
        assert!(ports.contains(8005));
        assert!(ports.contains(8010));
    }

    #[test]
    fn as_ports_err() {
        // Empty array
        let yaml = Yaml::Array(vec![]);
        let ports = as_ports(&yaml).unwrap();
        // Verify the ports collection is empty by checking a specific port
        assert!(!ports.contains(80));
        assert!(!ports.contains(443));
        assert!(!ports.contains(8080));

        // Array with invalid element
        let yaml = Yaml::Array(vec![Yaml::Integer(80), Yaml::Boolean(true)]);
        assert!(as_ports(&yaml).is_err());

        // Invalid type
        let yaml = Yaml::Null;
        assert!(as_ports(&yaml).is_err());
    }

    #[test]
    fn as_port_range_ok() {
        // Valid string range
        let yaml = yaml_str!("8000-8010");
        let range = as_port_range(&yaml).unwrap();
        assert_eq!(range.start(), 8000);
        assert_eq!(range.end(), 8010);

        // Valid hash with start/end
        let mut map = yaml_rust::yaml::Hash::new();
        map.insert(yaml_str!("start"), Yaml::Integer(9000));
        map.insert(yaml_str!("end"), Yaml::Integer(9010));
        let yaml = Yaml::Hash(map);
        let range = as_port_range(&yaml).unwrap();
        assert_eq!(range.start(), 9000);
        assert_eq!(range.end(), 9010);

        // Valid hash with from/to
        let mut map = yaml_rust::yaml::Hash::new();
        map.insert(yaml_str!("from"), Yaml::Integer(10000));
        map.insert(yaml_str!("to"), Yaml::Integer(10010));
        let yaml = Yaml::Hash(map);
        let range = as_port_range(&yaml).unwrap();
        assert_eq!(range.start(), 10000);
        assert_eq!(range.end(), 10010);
    }

    #[test]
    fn as_port_range_err() {
        // Invalid string range
        let yaml = yaml_str!("invalid-range");
        assert!(as_port_range(&yaml).is_err());

        // Hash missing required fields
        let mut map = yaml_rust::yaml::Hash::new();
        map.insert(yaml_str!("start"), Yaml::Integer(11000));
        let yaml = Yaml::Hash(map);
        assert!(as_port_range(&yaml).is_err());

        // Invalid port values
        let mut map = yaml_rust::yaml::Hash::new();
        map.insert(yaml_str!("start"), yaml_str!("invalid"));
        map.insert(yaml_str!("end"), Yaml::Integer(65536));
        let yaml = Yaml::Hash(map);
        assert!(as_port_range(&yaml).is_err());

        // Start > end
        let mut map = yaml_rust::yaml::Hash::new();
        map.insert(yaml_str!("start"), Yaml::Integer(13000));
        map.insert(yaml_str!("end"), Yaml::Integer(12900));
        let yaml = Yaml::Hash(map);
        assert!(as_port_range(&yaml).is_err());

        // Invalid type
        let yaml = Yaml::Integer(8080);
        assert!(as_port_range(&yaml).is_err());
    }
}
