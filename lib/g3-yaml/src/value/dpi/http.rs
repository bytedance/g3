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

#[cfg(test)]
#[cfg(feature = "dpi")]
mod tests {
    use super::*;
    use std::time::Duration;
    use yaml_rust::YamlLoader;

    #[test]
    fn as_h1_interception_config_ok() {
        // full valid configuration
        let yaml = yaml_doc!(
            r"
                pipeline_size: 10
                pipeline_read_idle_timeout: 300s
                req_header_recv_timeout: 30s
                rsp_header_recv_timeout: 60s
                req_header_max_size: 64KB
                rsp_header_max_size: 64KB
                body_line_max_length: 8192
                steal_forwarded_for: true
            "
        );
        let config = as_h1_interception_config(&yaml).unwrap();
        assert_eq!(config.pipeline_size.get(), 10);
        assert_eq!(config.pipeline_read_idle_timeout, Duration::from_secs(300));
        assert_eq!(config.req_head_recv_timeout, Duration::from_secs(30));
        assert_eq!(config.rsp_head_recv_timeout, Duration::from_secs(60));
        assert_eq!(config.req_head_max_size, 64000);
        assert_eq!(config.rsp_head_max_size, 64000);
        assert_eq!(config.body_line_max_len, 8192);
        assert_eq!(config.steal_forwarded_for, true);

        // default configuration
        let yaml = Yaml::Hash(Default::default());
        let config = as_h1_interception_config(&yaml).unwrap();
        assert_eq!(config.pipeline_size.get(), 10);
        assert_eq!(config.pipeline_read_idle_timeout, Duration::from_secs(300));
        assert_eq!(config.req_head_recv_timeout, Duration::from_secs(30));
        assert_eq!(config.rsp_head_recv_timeout, Duration::from_secs(60));
        assert_eq!(config.req_head_max_size, 65536);
        assert_eq!(config.rsp_head_max_size, 65536);
        assert_eq!(config.body_line_max_len, 8192);
        assert_eq!(config.steal_forwarded_for, false);

        // partial configuration with default values
        let yaml = yaml_doc!(
            r"
                pipeline_size: 5
                req_header_max_size: 32KB
            "
        );
        let config = as_h1_interception_config(&yaml).unwrap();
        assert_eq!(config.pipeline_size.get(), 5);
        assert_eq!(config.req_head_max_size, 32000);
        assert_eq!(config.steal_forwarded_for, false); // default value

        // boundary values
        let yaml = yaml_doc!(
            r"
                pipeline_size: 1
                body_line_max_length: 0
            "
        );
        let config = as_h1_interception_config(&yaml).unwrap();
        assert_eq!(config.pipeline_size.get(), 1);
        assert_eq!(config.body_line_max_len, 0);
    }

    #[test]
    fn as_h1_interception_config_err() {
        // invalid key
        let yaml = yaml_doc!(
            r"
                invalid_key: value
            "
        );
        assert!(as_h1_interception_config(&yaml).is_err());

        // invalid value for pipeline_size
        let yaml = yaml_doc!(
            r"
                pipeline_size: invalid
            "
        );
        assert!(as_h1_interception_config(&yaml).is_err());

        // invalid value for pipeline_read_idle_timeout
        let yaml = yaml_doc!(
            r"
                pipeline_read_idle_timeout: 300x
            "
        );
        assert!(as_h1_interception_config(&yaml).is_err());

        // invalid value for req_header_recv_timeout
        let yaml = yaml_doc!(
            r"
                req_header_recv_timeout: 30x
            "
        );
        assert!(as_h1_interception_config(&yaml).is_err());

        // invalid value for rsp_header_recv_timeout
        let yaml = yaml_doc!(
            r"
                rsp_header_recv_timeout: -1s
            "
        );
        assert!(as_h1_interception_config(&yaml).is_err());

        // invalid value for req_header_max_size
        let yaml = yaml_doc!(
            r"
                req_header_max_size: 64KBX
            "
        );
        assert!(as_h1_interception_config(&yaml).is_err());

        // invalid value for rsp_header_max_size
        let yaml = yaml_doc!(
            r"
                rsp_header_max_size: -1KB
            "
        );
        assert!(as_h1_interception_config(&yaml).is_err());

        // invalid value for body_line_max_length
        let yaml = yaml_doc!(
            r"
                body_line_max_length: -1
            "
        );
        assert!(as_h1_interception_config(&yaml).is_err());

        // invalid value for steal_forwarded_for
        let yaml = yaml_doc!(
            r"
                steal_forwarded_for: not_bool
            "
        );
        assert!(as_h1_interception_config(&yaml).is_err());

        // non-map input
        let yaml = Yaml::Array(vec![]);
        assert!(as_h1_interception_config(&yaml).is_err());
    }

    #[test]
    fn as_h2_interception_config_ok() {
        // full valid configuration
        let yaml = yaml_doc!(
            r"
                max_header_list_size: 64KB
                max_concurrent_streams: 128
                max_frame_size: 256KB
                stream_window_size: 1MB
                connection_window_size: 2MB
                max_send_buffer_size: 8MB
                upstream_handshake_timeout: 10s
                upstream_stream_open_timeout: 10s
                client_handshake_timeout: 4s
                ping_interval: 60s
                rsp_header_recv_timeout: 60s
                silent_drop_expect_header: true
            "
        );
        let config = as_h2_interception_config(&yaml).unwrap();
        assert_eq!(config.max_header_list_size, 64000);
        assert_eq!(config.max_concurrent_streams, 128);
        assert_eq!(config.max_frame_size(), 256_000);
        assert_eq!(config.stream_window_size(), 1_000_000);
        assert_eq!(config.connection_window_size(), 2_000_000);
        assert_eq!(config.max_send_buffer_size, 8_000_000);
        assert_eq!(config.upstream_handshake_timeout, Duration::from_secs(10));
        assert_eq!(config.upstream_stream_open_timeout, Duration::from_secs(10));
        assert_eq!(config.client_handshake_timeout, Duration::from_secs(4));
        assert_eq!(config.ping_interval, Duration::from_secs(60));
        assert_eq!(config.rsp_head_recv_timeout, Duration::from_secs(60));
        assert_eq!(config.silent_drop_expect_header, true);

        // alias key (max_header_size)
        let yaml = yaml_doc!(
            r"
                max_header_size: 32KB
            "
        );
        let config = as_h2_interception_config(&yaml).unwrap();
        assert_eq!(config.max_header_list_size, 32000);

        // default configuration
        let yaml = Yaml::Hash(Default::default());
        let config = as_h2_interception_config(&yaml).unwrap();
        assert_eq!(config.max_header_list_size, 64 * 1024);
        assert_eq!(config.max_concurrent_streams, 128);
        assert_eq!(config.max_frame_size(), 256 * 1024);
        assert_eq!(config.stream_window_size(), 1024 * 1024);
        assert_eq!(config.connection_window_size(), 2 * 1024 * 1024);
        assert_eq!(config.max_send_buffer_size, 8 * 1024 * 1024);
        assert_eq!(config.upstream_handshake_timeout, Duration::from_secs(10));
        assert_eq!(config.upstream_stream_open_timeout, Duration::from_secs(10));
        assert_eq!(config.client_handshake_timeout, Duration::from_secs(4));
        assert_eq!(config.ping_interval, Duration::from_secs(60));
        assert_eq!(config.rsp_head_recv_timeout, Duration::from_secs(60));
        assert_eq!(config.silent_drop_expect_header, false);

        // value clamping
        let yaml = yaml_doc!(
            r"
                max_frame_size: 1000  # below min, should clamp to 16384
                stream_window_size: 1000  # below min, should clamp to 65536
            "
        );
        let config = as_h2_interception_config(&yaml).unwrap();
        assert_eq!(config.max_frame_size(), 16384);
        assert_eq!(config.stream_window_size(), 65536);
    }

    #[test]
    fn as_h2_interception_config_err() {
        // invalid key
        let yaml = yaml_doc!(
            r"
                invalid_key: value
            "
        );
        assert!(as_h2_interception_config(&yaml).is_err());

        // invalid value for max_header_list_size
        let yaml = yaml_doc!(
            r"
                max_header_list_size: -1KB
            "
        );
        assert!(as_h2_interception_config(&yaml).is_err());

        // invalid value for max_concurrent_streams
        let yaml = yaml_doc!(
            r"
                max_concurrent_streams: -1
            "
        );
        assert!(as_h2_interception_config(&yaml).is_err());

        // invalid value for max_frame_size
        let yaml = yaml_doc!(
            r"
                max_frame_size: u32::MAX + 1
            "
        );
        assert!(as_h2_interception_config(&yaml).is_err());

        // invalid value for stream_window_size
        let yaml = yaml_doc!(
            r"
                stream_window_size: invalid
            "
        );
        assert!(as_h2_interception_config(&yaml).is_err());

        // invalid value for connection_window_size
        let yaml = yaml_doc!(
            r"
                connection_window_size: -1MB
            "
        );
        assert!(as_h2_interception_config(&yaml).is_err());

        // invalid value for max_send_buffer_size
        let yaml = yaml_doc!(
            r"
                max_send_buffer_size: 4x
            "
        );
        assert!(as_h2_interception_config(&yaml).is_err());

        // invalid value for upstream_handshake_timeout
        let yaml = yaml_doc!(
            r"
                upstream_handshake_timeout: -1s
            "
        );
        assert!(as_h2_interception_config(&yaml).is_err());

        // invalid value for upstream_stream_open_timeout
        let yaml = yaml_doc!(
            r"
                upstream_stream_open_timeout: invalid
            "
        );
        assert!(as_h2_interception_config(&yaml).is_err());

        // invalid value for client_handshake_timeout
        let yaml = yaml_doc!(
            r"
                client_handshake_timeout: 1x
            "
        );
        assert!(as_h2_interception_config(&yaml).is_err());

        // invalid value for ping_interval
        let yaml = yaml_doc!(
            r"
                ping_interval: -1s
            "
        );
        assert!(as_h2_interception_config(&yaml).is_err());

        // invalid value for rsp_header_recv_timeout
        let yaml = yaml_doc!(
            r"
                rsp_header_recv_timeout: 10x
            "
        );
        assert!(as_h2_interception_config(&yaml).is_err());

        // invalid value for silent_drop_expect_header
        let yaml = yaml_doc!(
            r"
                silent_drop_expect_header: not_bool
            "
        );
        assert!(as_h2_interception_config(&yaml).is_err());

        // non-map input
        let yaml = Yaml::Array(vec![]);
        assert!(as_h2_interception_config(&yaml).is_err());
    }
}
