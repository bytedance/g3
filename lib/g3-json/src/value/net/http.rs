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
