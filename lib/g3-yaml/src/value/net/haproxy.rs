/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use yaml_rust::Yaml;

use anyhow::{Context, anyhow};
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_as_proxy_protocol_version() {
        let yaml = Yaml::String("1".to_string());
        let version = as_proxy_protocol_version(&yaml).unwrap();
        assert_eq!(version, ProxyProtocolVersion::V1);

        let yaml = Yaml::Integer(2);
        let version = as_proxy_protocol_version(&yaml).unwrap();
        assert_eq!(version, ProxyProtocolVersion::V2);

        let yaml = Yaml::String("3".to_string()); // Invalid version
        assert!(as_proxy_protocol_version(&yaml).is_err());

        let yaml = Yaml::Integer(256); // Beyond u8 range
        assert!(as_proxy_protocol_version(&yaml).is_err());

        let yaml = Yaml::Boolean(true); // Invalid type
        assert!(as_proxy_protocol_version(&yaml).is_err());

        let yaml = Yaml::Array(vec![]); // Invalid type
        assert!(as_proxy_protocol_version(&yaml).is_err());

        let yaml = Yaml::Null; // Invalid type
        assert!(as_proxy_protocol_version(&yaml).is_err())
    }
}
