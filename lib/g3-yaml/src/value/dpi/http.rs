/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_dpi::{H1InterceptionConfig, H2InterceptionConfig};

pub fn as_h1_interception_config(value: &Yaml) -> anyhow::Result<H1InterceptionConfig> {
    if let Yaml::Hash(map) = value {
        let mut config = H1InterceptionConfig::default();

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "pipeline_size" => {
                config.pipeline_size = crate::value::as_nonzero_usize(v)?;
                Ok(())
            }
            "pipeline_read_idle_timeout" => {
                config.pipeline_read_idle_timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "req_header_recv_timeout" => {
                config.req_head_recv_timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "rsp_header_recv_timeout" => {
                config.rsp_head_recv_timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "req_header_max_size" => {
                config.req_head_max_size = crate::humanize::as_usize(v)
                    .context(format!("invalid humanize usize value for key {k}"))?;
                Ok(())
            }
            "rsp_header_max_size" => {
                config.rsp_head_max_size = crate::humanize::as_usize(v)
                    .context(format!("invalid humanize usize value for key {k}"))?;
                Ok(())
            }
            "body_line_max_length" => {
                config.body_line_max_len = crate::value::as_usize(v)
                    .context(format!("invalid usize value for key {k}"))?;
                Ok(())
            }
            "steal_forwarded_for" => {
                config.steal_forwarded_for = crate::value::as_bool(v)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        Ok(config)
    } else {
        Err(anyhow!(
            "yaml value type for 'h1 interception config' should be 'map'"
        ))
    }
}

pub fn as_h2_interception_config(value: &Yaml) -> anyhow::Result<H2InterceptionConfig> {
    if let Yaml::Hash(map) = value {
        let mut config = H2InterceptionConfig::default();

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "max_header_list_size" | "max_header_size" => {
                config.max_header_list_size = crate::humanize::as_u32(v)
                    .context(format!("invalid humanize u32 value for key {k}"))?;
                Ok(())
            }
            "max_concurrent_streams" => {
                config.max_concurrent_streams = crate::value::as_u32(v)?;
                Ok(())
            }
            "max_frame_size" => {
                let max_frame_size = crate::humanize::as_u32(v)
                    .context(format!("invalid humanize u32 value for key {k}"))?;
                config.set_max_frame_size(max_frame_size);
                Ok(())
            }
            "stream_window_size" => {
                let window_size = crate::humanize::as_u32(v)
                    .context(format!("invalid humanize u32 value for key {k}"))?;
                config.set_stream_window_size(window_size);
                Ok(())
            }
            "connection_window_size" => {
                let window_size = crate::humanize::as_u32(v)
                    .context(format!("invalid humanize u32 value for key {k}"))?;
                config.set_connection_window_size(window_size);
                Ok(())
            }
            "max_send_buffer_size" => {
                config.max_send_buffer_size = crate::humanize::as_usize(v)
                    .context(format!("invalid humanize usize value for key {k}"))?;
                Ok(())
            }
            "upstream_handshake_timeout" => {
                config.upstream_handshake_timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "upstream_stream_open_timeout" => {
                config.upstream_stream_open_timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "client_handshake_timeout" => {
                config.client_handshake_timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "ping_interval" => {
                config.ping_interval = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "rsp_header_recv_timeout" => {
                config.rsp_head_recv_timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "silent_drop_expect_header" => {
                config.silent_drop_expect_header = crate::value::as_bool(v)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        Ok(config)
    } else {
        Err(anyhow!(
            "yaml value type for 'h2 interception config' should be 'map'"
        ))
    }
}
