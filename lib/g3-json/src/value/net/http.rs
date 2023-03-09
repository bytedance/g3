/*
 * Copyright 2023 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use anyhow::{anyhow, Context};
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
