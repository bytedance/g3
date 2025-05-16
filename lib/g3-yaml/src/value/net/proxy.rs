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
