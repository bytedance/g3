/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_dpi::ImapInterceptionConfig;

pub fn as_imap_interception_config(value: &Yaml) -> anyhow::Result<ImapInterceptionConfig> {
    if let Yaml::Hash(map) = value {
        let mut config = ImapInterceptionConfig::default();

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "greeting_timeout" => {
                config.greeting_timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "authenticate_timeout" => {
                config.authenticate_timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "logout_wait_timeout" => {
                config.logout_wait_timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "command_line_max_size" => {
                config.command_line_max_size = crate::value::as_usize(v)?;
                Ok(())
            }
            "response_line_max_size" => {
                config.response_line_max_size = crate::value::as_usize(v)?;
                Ok(())
            }
            "forward_max_idle_count" => {
                config.forward_max_idle_count = crate::value::as_usize(v)?;
                Ok(())
            }
            "transfer_max_idle_count" => {
                config.transfer_max_idle_count = crate::value::as_usize(v)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        Ok(config)
    } else {
        Err(anyhow!(
            "yaml value type for 'imap interception config' should be 'map'"
        ))
    }
}

#[cfg(test)]
#[cfg(feature = "dpi")]
mod test {
    use super::*;
    use std::time::Duration;
    use yaml_rust::YamlLoader;

    #[test]
    fn as_imap_interception_config_ok() {
        // full valid configuration
        let yaml = yaml_doc!(
            r"
                greeting_timeout: 10s
                authenticate_timeout: 5m
                logout_wait_timeout: 3s
                command_line_max_size: 2048
                response_line_max_size: 4096
                forward_max_idle_count: 20
                transfer_max_idle_count: 3
            "
        );
        let config = as_imap_interception_config(&yaml).unwrap();
        assert_eq!(config.greeting_timeout, Duration::from_secs(10));
        assert_eq!(config.authenticate_timeout, Duration::from_secs(300));
        assert_eq!(config.logout_wait_timeout, Duration::from_secs(3));
        assert_eq!(config.command_line_max_size, 2048);
        assert_eq!(config.response_line_max_size, 4096);
        assert_eq!(config.forward_max_idle_count, 20);
        assert_eq!(config.transfer_max_idle_count, 3);

        // default configuration
        let yaml = Yaml::Hash(Default::default());
        let config = as_imap_interception_config(&yaml).unwrap();
        assert_eq!(config.greeting_timeout, Duration::from_secs(300));
        assert_eq!(config.authenticate_timeout, Duration::from_secs(300));
        assert_eq!(config.logout_wait_timeout, Duration::from_secs(10));
        assert_eq!(config.command_line_max_size, 4096);
        assert_eq!(config.response_line_max_size, 4096);
        assert_eq!(config.forward_max_idle_count, 30);
        assert_eq!(config.transfer_max_idle_count, 5);
    }

    #[test]
    fn as_imap_interception_config_err() {
        // invalid value for greeting_timeout
        let yaml = yaml_doc!(
            r"
                greeting_timeout: invalid
            "
        );
        assert!(as_imap_interception_config(&yaml).is_err());

        // invalid value for authenticate_timeout
        let yaml = yaml_doc!(
            r"
                authenticate_timeout: -1s
            "
        );
        assert!(as_imap_interception_config(&yaml).is_err());

        // invalid value for logout_wait_timeout
        let yaml = yaml_doc!(
            r"
                logout_wait_timeout: 10x
            "
        );
        assert!(as_imap_interception_config(&yaml).is_err());

        // invalid value for command_line_max_size
        let yaml = yaml_doc!(
            r"
                command_line_max_size: invalid
            "
        );
        assert!(as_imap_interception_config(&yaml).is_err());

        // invalid value for response_line_max_size
        let yaml = yaml_doc!(
            r"
                response_line_max_size: -1
            "
        );
        assert!(as_imap_interception_config(&yaml).is_err());

        // invalid value for forward_max_idle_count
        let yaml = yaml_doc!(
            r"
                forward_max_idle_count: invalid
            "
        );
        assert!(as_imap_interception_config(&yaml).is_err());

        // invalid value for transfer_max_idle_count
        let yaml = yaml_doc!(
            r"
                transfer_max_idle_count: 1x
            "
        );
        assert!(as_imap_interception_config(&yaml).is_err());

        // invalid key
        let yaml = yaml_doc!(
            r"
                invalid_key: value
            "
        );
        assert!(as_imap_interception_config(&yaml).is_err());

        // non-map input
        let yaml = yaml_str!("invalid");
        assert!(as_imap_interception_config(&yaml).is_err());

        let yaml = Yaml::Array(vec![]);
        assert!(as_imap_interception_config(&yaml).is_err());
    }
}
