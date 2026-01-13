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
                    let ttl =
                        crate::value::as_u8(v).context(format!("invalid u8 value for key {k}"))?;
                    config.time_to_live = Some(ttl);
                }
                "hop_limit" => {
                    let hops =
                        crate::value::as_u8(v).context(format!("invalid u8 value for key {k}"))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn as_udp_misc_sock_opts_ok() {
        // all valid keys including aliases
        let valid_json = json!({
            "time_to_live": 64,
            "hop_limit": 32,
            "type_of_service": 8,
        });

        let config = as_udp_misc_sock_opts(&valid_json).unwrap();
        assert_eq!(config.time_to_live, Some(64));
        assert_eq!(config.hop_limit, Some(32));
        assert_eq!(config.type_of_service, Some(8));

        // platform-specific tests
        #[cfg(not(windows))]
        {
            let traffic_json = json!({"traffic_class": 3});
            let config = as_udp_misc_sock_opts(&traffic_json).unwrap();
            assert_eq!(config.traffic_class, Some(3));
        }

        #[cfg(target_os = "linux")]
        {
            let mark_json = json!({"netfilter_mark": 99});
            let config = as_udp_misc_sock_opts(&mark_json).unwrap();
            assert_eq!(config.netfilter_mark, Some(99));
        }
    }

    #[test]
    fn as_udp_misc_sock_opts_err() {
        // non-object input
        let array_input = json!([1, 2, 3]);
        assert!(as_udp_misc_sock_opts(&array_input).is_err());

        // invalid key
        let invalid_key = json!({"invalid_key": 100});
        assert!(as_udp_misc_sock_opts(&invalid_key).is_err());

        // type error
        let type_error = json!({"ttl": "not_a_number"});
        assert!(type_error.get("ttl").unwrap().is_string());
        assert!(as_udp_misc_sock_opts(&type_error).is_err());
    }
}
