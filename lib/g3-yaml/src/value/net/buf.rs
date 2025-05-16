/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_types::net::SocketBufferConfig;

pub fn as_socket_buffer_config(value: &Yaml) -> anyhow::Result<SocketBufferConfig> {
    let mut config = SocketBufferConfig::default();

    match value {
        Yaml::Integer(_) | Yaml::String(_) => {
            let size =
                crate::humanize::as_usize(value).context("invalid single humanize usize value")?;
            config.set_recv_size(size);
            config.set_send_size(size);
        }
        Yaml::Hash(map) => {
            crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
                "recv" | "receive" | "read" => {
                    let size = crate::humanize::as_usize(v)
                        .context(format!("invalid humanize usize value for key {k}"))?;
                    config.set_recv_size(size);
                    Ok(())
                }
                "send" | "write" => {
                    let size = crate::humanize::as_usize(v)
                        .context(format!("invalid humanize usize value for key {k}"))?;
                    config.set_send_size(size);
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;
        }
        _ => return Err(anyhow!("invalid yaml value: {:?}", value)),
    }

    Ok(config)
}
