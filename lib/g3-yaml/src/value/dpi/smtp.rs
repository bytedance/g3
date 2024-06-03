/*
 * Copyright 2024 ByteDance and/or its affiliates.
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
use yaml_rust::Yaml;

use g3_dpi::SmtpInterceptionConfig;

pub fn as_smtp_interception_config(value: &Yaml) -> anyhow::Result<SmtpInterceptionConfig> {
    if let Yaml::Hash(map) = value {
        let mut config = SmtpInterceptionConfig::default();

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "greeting_timeout" => {
                config.greeting_timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "quit_wait_timeout" => {
                config.quit_wait_timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "allow_on_demand_mail_relay" | "allow_odmr" => {
                config.allow_on_demand_mail_relay = crate::value::as_bool(v)?;
                Ok(())
            }
            "allow_burl_data" | "allow_burl" => {
                config.allow_burl_data = crate::value::as_bool(v)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        Ok(config)
    } else {
        Err(anyhow!(
            "yaml value type for 'smtp interception config' should be 'map'"
        ))
    }
}
