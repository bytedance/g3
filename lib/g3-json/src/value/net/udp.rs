/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use serde_json::Value;

use g3_types::net::UdpMiscSockOpts;

pub fn as_udp_misc_sock_opts(v: &Value) -> anyhow::Result<UdpMiscSockOpts> {
    let mut config = UdpMiscSockOpts::default();

    if let Value::Object(map) = v {
        for (k, v) in map {
            match crate::key::normalize(k).as_str() {
                "time_to_live" | "ttl" => {
                    let ttl = crate::value::as_u32(v)
                        .context(format!("invalid u32 value for key {k}"))?;
                    config.time_to_live = Some(ttl);
                }
                "hop_limit" => {
                    let hops = crate::value::as_u32(v)
                        .context(format!("invalid u32 value for key {k}"))?;
                    config.hop_limit = Some(hops);
                }
                "type_of_service" | "tos" => {
                    let tos =
                        crate::value::as_u8(v).context(format!("invalid u8 value for key {k}"))?;
                    config.type_of_service = Some(tos);
                }
                #[cfg(not(windows))]
                "traffic_class" => {
                    let class =
                        crate::value::as_u8(v).context(format!("invalid u8 value for key {k}"))?;
                    config.traffic_class = Some(class);
                }
                #[cfg(target_os = "linux")]
                "netfilter_mark" | "mark" => {
                    let mark = crate::value::as_u32(v)
                        .context(format!("invalid u32 value for key {k}"))?;
                    config.netfilter_mark = Some(mark);
                }
                _ => return Err(anyhow!("invalid key {k}")),
            }
        }

        Ok(config)
    } else {
        Err(anyhow!(
            "json value type for 'UdpMiscSockOpts' should be 'map'"
        ))
    }
}
