/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
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
            "command_wait_timeout" => {
                config.command_wait_timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "response_wait_timeout" => {
                config.response_wait_timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "data_initiation_timeout" => {
                config.data_initiation_timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "data_termination_timeout" => {
                config.data_termination_timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "allow_on_demand_mail_relay" | "allow_odmr" => {
                config.allow_on_demand_mail_relay = crate::value::as_bool(v)?;
                Ok(())
            }
            "allow_data_chunking" => {
                config.allow_data_chunking = crate::value::as_bool(v)?;
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

#[cfg(test)]
#[cfg(feature = "dpi")]
mod test {
    use super::*;
    use std::time::Duration;
    use yaml_rust::YamlLoader;

    #[test]
    fn as_smtp_interception_config_ok() {
        // full valid configuation
        let yaml = yaml_doc!(
            r"
                greeting_timeout: 10s
                quit_wait_timeout: 5s
                command_wait_timeout: 5m
                response_wait_timeout: 30s
                data_initiation_timeout: 20s
                data_termination_timeout: 50s
                allow_on_demand_mail_relay: true
                allow_data_chunking: true
                allow_burl_data: true
            "
        );
        let config = as_smtp_interception_config(&yaml).unwrap();
        assert_eq!(config.greeting_timeout, Duration::from_secs(10));
        assert_eq!(config.quit_wait_timeout, Duration::from_secs(5));
        assert_eq!(config.command_wait_timeout, Duration::from_secs(300));
        assert_eq!(config.response_wait_timeout, Duration::from_secs(30));
        assert_eq!(config.data_initiation_timeout, Duration::from_secs(20));
        assert_eq!(config.data_termination_timeout, Duration::from_secs(50));
        assert_eq!(config.allow_on_demand_mail_relay, true);
        assert_eq!(config.allow_data_chunking, true);
        assert_eq!(config.allow_burl_data, true);

        // alias key (allow_odmr and allow_burl)
        let yaml = yaml_doc!(
            r"
                allow_odmr: true
                allow_burl: true
            "
        );
        let config = as_smtp_interception_config(&yaml).unwrap();
        assert_eq!(config.allow_on_demand_mail_relay, true);
        assert_eq!(config.allow_burl_data, true);

        // default configuation
        let yaml = Yaml::Hash(Default::default());
        let config = as_smtp_interception_config(&yaml).unwrap();
        assert_eq!(config.greeting_timeout, Duration::from_secs(300));
        assert_eq!(config.quit_wait_timeout, Duration::from_secs(60));
        assert_eq!(config.command_wait_timeout, Duration::from_secs(300));
        assert_eq!(config.response_wait_timeout, Duration::from_secs(300));
        assert_eq!(config.data_initiation_timeout, Duration::from_secs(120));
        assert_eq!(config.data_termination_timeout, Duration::from_secs(600));
        assert_eq!(config.allow_on_demand_mail_relay, false);
        assert_eq!(config.allow_data_chunking, false);
        assert_eq!(config.allow_burl_data, false);
    }

    #[test]
    fn as_smtp_interception_config_err() {
        // invalid value for greeting_timeout
        let yaml = yaml_doc!(
            r"
                greeting_timeout: invalid
            "
        );
        assert!(as_smtp_interception_config(&yaml).is_err());

        // invalid value for quit_wait_timeout
        let yaml = yaml_doc!(
            r"
                quit_wait_timeout: -1s
            "
        );
        assert!(as_smtp_interception_config(&yaml).is_err());

        // invalid value for command_wait_timeout
        let yaml = yaml_doc!(
            r"
                command_wait_timeout: 1x
            "
        );
        assert!(as_smtp_interception_config(&yaml).is_err());

        // invalid value for response_wait_timeout
        let yaml = yaml_doc!(
            r"
                response_wait_timeout: invalid
            "
        );
        assert!(as_smtp_interception_config(&yaml).is_err());

        // invalid value for data_initiation_timeout
        let yaml = yaml_doc!(
            r"
                data_initiation_timeout: -5s
            "
        );
        assert!(as_smtp_interception_config(&yaml).is_err());

        // invalid value for data_termination_timeout
        let yaml = yaml_doc!(
            r"
                data_termination_timeout: 5y
            "
        );
        assert!(as_smtp_interception_config(&yaml).is_err());

        // invalid value for allow_on_demand_mail_relay
        let yaml = yaml_doc!(
            r"
                allow_on_demand_mail_relay: invalid
            "
        );
        assert!(as_smtp_interception_config(&yaml).is_err());

        // invalid value for allow_data_chunking
        let yaml = yaml_doc!(
            r"
                allow_data_chunking: not_bool
            "
        );
        assert!(as_smtp_interception_config(&yaml).is_err());

        // invalid value for allow_burl_data
        let yaml = yaml_doc!(
            r"
                allow_burl_data: invalid_bool
            "
        );
        assert!(as_smtp_interception_config(&yaml).is_err());

        // invalid key
        let yaml = yaml_doc!(
            r"
                invalid_key: value
            "
        );
        assert!(as_smtp_interception_config(&yaml).is_err());

        // non-map input
        let yaml = yaml_str!("invalid");
        assert!(as_smtp_interception_config(&yaml).is_err());

        let yaml = Yaml::Array(vec![]);
        assert!(as_smtp_interception_config(&yaml).is_err());
    }
}
