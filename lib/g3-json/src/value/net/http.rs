/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use serde_json::Value;

use g3_types::net::HttpKeepAliveConfig;

pub fn as_http_keepalive_config(v: &Value) -> anyhow::Result<HttpKeepAliveConfig> {
    let mut config = HttpKeepAliveConfig::default();

    match v {
        Value::Object(map) => {
            for (k, v) in map {
                match crate::key::normalize(k).as_str() {
                    "enable" => {
                        let enable = crate::value::as_bool(v)
                            .context(format!("invalid boolean value for key {k}"))?;
                        config.set_enable(enable);
                    }
                    "idle_expire" => {
                        let idle_expire = crate::humanize::as_duration(v)
                            .context(format!("invalid humanize duration value for key {k}"))?;
                        config.set_idle_expire(idle_expire);
                    }
                    _ => return Err(anyhow!("invalid key {k}")),
                }
            }
        }
        Value::Bool(enable) => {
            config.set_enable(*enable);
        }
        _ => {
            let idle_expire = crate::humanize::as_duration(v)
                .context("invalid http keepalive idle_expire value")?;
            config.set_enable(true);
            config.set_idle_expire(idle_expire);
        }
    }

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::Duration;

    #[test]
    fn as_http_keepalive_config_ok() {
        // boolean input
        let v = json!(true);
        let config = as_http_keepalive_config(&v).unwrap();
        assert!(config.is_enabled());
        assert_eq!(config.idle_expire(), Duration::from_secs(60));

        let v = json!(false);
        let config = as_http_keepalive_config(&v).unwrap();
        assert!(!config.is_enabled());
        assert_eq!(config.idle_expire(), Duration::ZERO);

        // duration string input
        let v = json!("30s");
        let config = as_http_keepalive_config(&v).unwrap();
        assert!(config.is_enabled());
        assert_eq!(config.idle_expire(), Duration::from_secs(30));

        // object with only "enable"
        let v = json!({"enable": true});
        let config = as_http_keepalive_config(&v).unwrap();
        assert!(config.is_enabled());
        assert_eq!(config.idle_expire(), Duration::from_secs(60));

        // object with only "idle_expire"
        let v = json!({"idle_expire": "45s"});
        let config = as_http_keepalive_config(&v).unwrap();
        assert!(config.is_enabled());
        assert_eq!(config.idle_expire(), Duration::from_secs(45));

        // object with both keys
        let v = json!({
            "enable": false,
            "idle_expire": "90s"
        });
        let config = as_http_keepalive_config(&v).unwrap();
        assert!(!config.is_enabled());
        assert_eq!(config.idle_expire(), Duration::ZERO);

        let v = json!({
            "enable": true,
            "idle_expire": "120s"
        });
        let config = as_http_keepalive_config(&v).unwrap();
        assert!(config.is_enabled());
        assert_eq!(config.idle_expire(), Duration::from_secs(120));
    }

    #[test]
    fn as_http_keepalive_config_err() {
        // invalid key in object
        let v = json!({"invalid_key": true});
        assert!(as_http_keepalive_config(&v).is_err());

        // wrong type for "enable"
        let v = json!({"enable": "not_a_boolean"});
        assert!(as_http_keepalive_config(&v).is_err());

        // wrong type for "idle_expire"
        let v = json!({"idle_expire": true});
        assert!(as_http_keepalive_config(&v).is_err());

        // invalid duration format
        let v = json!("invalid_duration");
        assert!(as_http_keepalive_config(&v).is_err());

        // unsupported type (array)
        let v = json!([1, 2, 3]);
        assert!(as_http_keepalive_config(&v).is_err());
    }
}
