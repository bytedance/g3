/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    use yaml_rust::YamlLoader;

    #[test]
    fn as_quinn_transport_config_ok() {
        // full configuration
        let yaml = yaml_doc!(
            r#"
                max_idle_timeout: "60s"
                keep_alive_interval: "10s"
                stream_receive_window: 65535
                receive_window: 131072
                send_window: 262144
            "#
        );
        let config = as_quinn_transport_config(&yaml).unwrap();
        let mut expected = QuinnTransportConfigBuilder::default();
        expected
            .set_max_idle_timeout(Duration::from_secs(60))
            .unwrap();
        expected.set_keep_alive_interval(Duration::from_secs(10));
        expected.set_stream_receive_window(65535);
        expected.set_receive_window(131072);
        expected.set_send_window(262144);
        assert_eq!(config, expected);

        // default configuration
        let yaml = Yaml::Hash(Default::default());
        let config = as_quinn_transport_config(&yaml).unwrap();
        let mut expected = QuinnTransportConfigBuilder::default();
        expected
            .set_max_idle_timeout(Duration::from_secs(60))
            .unwrap();
        expected.set_keep_alive_interval(Duration::from_secs(10));
        assert_eq!(config, expected);

        // boundary values
        let yaml = yaml_doc!(
            r#"
                stream_receive_window: 1
                receive_window: 1
                send_window: 1
            "#
        );
        let config = as_quinn_transport_config(&yaml).unwrap();
        let mut expected = QuinnTransportConfigBuilder::default();
        expected.set_stream_receive_window(1);
        expected.set_receive_window(1);
        expected.set_send_window(1);
        assert_eq!(config, expected);
    }

    #[test]
    fn as_quinn_transport_config_err() {
        // invalid key
        let yaml = yaml_doc!(
            r#"
                invalid_key: "value"
            "#
        );
        assert!(as_quinn_transport_config(&yaml).is_err());

        // non-hash input
        let yaml = yaml_doc!(
            r#"
                - item1
                - item2
            "#
        );
        assert!(as_quinn_transport_config(&yaml).is_err());

        let yaml = Yaml::Boolean(true);
        assert!(as_quinn_transport_config(&yaml).is_err());

        // invalid value type
        let yaml = yaml_doc!(
            r#"
                max_idle_timeout: "invalid"
            "#
        );
        assert!(as_quinn_transport_config(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                keep_alive_interval: "1x"
            "#
        );
        assert!(as_quinn_transport_config(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                stream_receive_window: "not_a_number"
            "#
        );
        assert!(as_quinn_transport_config(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                receive_window: -1
            "#
        );
        assert!(as_quinn_transport_config(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                send_window: 1y
            "#
        );
        assert!(as_quinn_transport_config(&yaml).is_err());
    }
}
