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

use g3_types::net::QuinnTransportConfigBuilder;

pub fn as_quinn_transport_config(value: &Yaml) -> anyhow::Result<QuinnTransportConfigBuilder> {
    let Yaml::Hash(map) = value else {
        return Err(anyhow!(
            "yaml value type for quinn transport config should be 'map'"
        ));
    };

    let mut config = QuinnTransportConfigBuilder::default();
    crate::hash::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
        "max_idle_timeout" => {
            let timeout = crate::humanize::as_duration(v)
                .context(format!("invalid humanize duration value for key {k}"))?;
            config
                .set_max_idle_timeout(timeout)
                .context("failed to set max idle timeout")
        }
        "keep_alive_interval" => {
            let interval = crate::humanize::as_duration(v)
                .context(format!("invalid humanize duration value for key {k}"))?;
            config.set_keep_alive_interval(interval);
            Ok(())
        }
        "stream_receive_window" => {
            let size = crate::humanize::as_u32(v)
                .context(format!("invalid humanize u32 value for key {k}"))?;
            config.set_stream_receive_window(size);
            Ok(())
        }
        "receive_window" => {
            let size = crate::humanize::as_u32(v)
                .context(format!("invalid humanize u32 value for key {k}"))?;
            config.set_receive_window(size);
            Ok(())
        }
        "send_window" => {
            let size = crate::humanize::as_u32(v)
                .context(format!("invalid humanize u32 value for key {k}"))?;
            config.set_send_window(size);
            Ok(())
        }
        _ => Err(anyhow!("invalid key {k}")),
    })?;
    Ok(config)
}
