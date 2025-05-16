/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::anyhow;
use serde_json::Value;

use g3_types::net::ProxyRequestType;

pub fn as_proxy_request_type(v: &Value) -> anyhow::Result<ProxyRequestType> {
    if let Value::String(s) = v {
        let t = ProxyRequestType::from_str(s)
            .map_err(|_| anyhow!("invalid 'ProxyRequestType' value"))?;
        Ok(t)
    } else {
        Err(anyhow!(
            "json value type for 'ProxyRequestType' should be 'string'"
        ))
    }
}
