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
