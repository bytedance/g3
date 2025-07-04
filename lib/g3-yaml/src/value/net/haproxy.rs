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
    fn as_proxy_protocol_version_ok() {
        let yaml = yaml_str!("1");
        assert_eq!(
            as_proxy_protocol_version(&yaml).unwrap(),
            ProxyProtocolVersion::V1
        );

        let yaml = Yaml::Integer(2);
        assert_eq!(
            as_proxy_protocol_version(&yaml).unwrap(),
            ProxyProtocolVersion::V2
        );
    }

    #[test]
    fn as_proxy_protocol_version_err() {
        let yaml = yaml_str!("3"); // Invalid version
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
