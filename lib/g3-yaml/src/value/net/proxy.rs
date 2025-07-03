/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::anyhow;
use yaml_rust::Yaml;

use g3_types::net::ProxyRequestType;

pub fn as_proxy_request_type(v: &Yaml) -> anyhow::Result<ProxyRequestType> {
    if let Yaml::String(s) = v {
        let t = ProxyRequestType::from_str(s)
            .map_err(|_| anyhow!("invalid ProxyRequestType string"))?;
        Ok(t)
    } else {
        Err(anyhow!(
            "yaml value type for 'ProxyRequestType' should be 'string'"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_proxy_request_type_ok() {
        // valid ProxyRequestType string
        let v = yaml_str!("http_forward");
        let t = as_proxy_request_type(&v).unwrap();
        assert_eq!(t, ProxyRequestType::HttpForward);
    }

    #[test]
    fn as_proxy_request_type_err() {
        // invalid ProxyRequestType string
        let v = yaml_str!("invalid_type");
        assert!(as_proxy_request_type(&v).is_err());

        // invalid yaml value type for 'ProxyRequestType' should be'string'
        let v = Yaml::Integer(123);
        assert!(as_proxy_request_type(&v).is_err());
    }
}
