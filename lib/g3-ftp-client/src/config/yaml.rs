/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use super::{FtpClientConfig, FtpControlConfig, FtpTransferConfig};

impl FtpControlConfig {
    pub fn parse_yaml(value: &Yaml) -> anyhow::Result<Self> {
        if let Yaml::Hash(map) = value {
            let mut config = FtpControlConfig::default();
            g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                "max_line_len" | "max_line_length" => {
                    config.max_line_len = g3_yaml::humanize::as_usize(v)
                        .context(format!("invalid humanize usize value for key {k}"))?;
                    Ok(())
                }
                "max_multi_lines" => {
                    config.max_multi_lines = g3_yaml::value::as_usize(v)
                        .context(format!("invalid usize value for key {k}"))?;
                    Ok(())
                }
                "command_timeout" => {
                    config.command_timeout = g3_yaml::humanize::as_duration(v)
                        .context(format!("invalid humanize duration value for key {k}"))?;
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;
            Ok(config)
        } else {
            Err(anyhow!("invalid yaml type"))
        }
    }
}

impl FtpTransferConfig {
    pub fn parse_yaml(value: &Yaml) -> anyhow::Result<Self> {
        if let Yaml::Hash(map) = value {
            let mut config = FtpTransferConfig::default();
            g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                "list_max_line_len" | "list_max_line_length" => {
                    config.list_max_line_len = g3_yaml::humanize::as_usize(v)
                        .context(format!("invalid humanize usize value for key {k}"))?;
                    Ok(())
                }
                "list_max_entries" => {
                    config.list_max_entries = g3_yaml::value::as_usize(v)
                        .context(format!("invalid usize value for key {k}"))?;
                    Ok(())
                }
                "list_all_timeout" => {
                    let timeout = g3_yaml::humanize::as_duration(v)
                        .context(format!("invalid humanize duration value for key {k}"))?;
                    config.set_list_all_timeout(timeout);
                    Ok(())
                }
                "end_wait_timeout" => {
                    config.end_wait_timeout = g3_yaml::humanize::as_duration(v)
                        .context(format!("invalid humanize duration value for key {k}"))?;
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;
            Ok(config)
        } else {
            Err(anyhow!("invalid yaml type"))
        }
    }
}

impl FtpClientConfig {
    pub fn parse_yaml(value: &Yaml) -> anyhow::Result<Self> {
        if let Yaml::Hash(map) = value {
            let mut config = FtpClientConfig::default();
            g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                "control" => {
                    config.control = FtpControlConfig::parse_yaml(v).context(format!(
                        "invalid ftp control connection config value for key {k}"
                    ))?;
                    Ok(())
                }
                "transfer" => {
                    config.transfer = FtpTransferConfig::parse_yaml(v).context(format!(
                        "invalid ftp transfer connection config value for key {k}"
                    ))?;
                    Ok(())
                }
                "connect_timeout" => {
                    config.connect_timeout = g3_yaml::humanize::as_duration(v)
                        .context(format!("invalid humanize duration value for key {k}"))?;
                    Ok(())
                }
                "greeting_timeout" => {
                    config.greeting_timeout = g3_yaml::humanize::as_duration(v)
                        .context(format!("invalid humanize duration value for key {k}"))?;
                    Ok(())
                }
                "always_try_epsv" => {
                    config.always_try_epsv = g3_yaml::value::as_bool(v)
                        .context(format!("invalid bool value for key {k}"))?;
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;
            Ok(config)
        } else {
            Err(anyhow!("invalid yaml type"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g3_yaml::{yaml_doc, yaml_str};
    use yaml_rust::YamlLoader;

    #[test]
    fn ftp_control_config_parse_ok() {
        let yaml = yaml_doc!(
            r#"
                max_line_len: "2KB"
                max_multi_lines: 256
                command_timeout: "30s"
            "#
        );
        let config = FtpControlConfig::parse_yaml(&yaml).unwrap();
        assert_eq!(config.max_line_len, 2000);
        assert_eq!(config.max_multi_lines, 256);
        assert_eq!(config.command_timeout, std::time::Duration::from_secs(30));

        let yaml = yaml_doc!(
            r#"
                max_line_length: "1KB"
            "#
        );
        let config = FtpControlConfig::parse_yaml(&yaml).unwrap();
        assert_eq!(config.max_line_len, 1000);
    }

    #[test]
    fn ftp_control_config_parse_err() {
        let yaml = yaml_doc!(
            r#"
                invalid_key: "value"
            "#
        );
        assert!(FtpControlConfig::parse_yaml(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                max_line_len: "2KBX"
            "#
        );
        assert!(FtpControlConfig::parse_yaml(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                max_multi_lines: -1
            "#
        );
        assert!(FtpControlConfig::parse_yaml(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                command_timeout: "invalid_duration"
            "#
        );
        assert!(FtpControlConfig::parse_yaml(&yaml).is_err());

        let yaml = yaml_str!("invalid");
        assert!(FtpControlConfig::parse_yaml(&yaml).is_err());
    }

    #[test]
    fn ftp_transfer_config_parse_ok() {
        let yaml = yaml_doc!(
            r#"
                list_max_line_len: "4KB"
                list_max_entries: 2048
                list_all_timeout: "5m"
                end_wait_timeout: "500ms"
            "#
        );
        let config = FtpTransferConfig::parse_yaml(&yaml).unwrap();
        assert_eq!(config.list_max_line_len, 4000);
        assert_eq!(config.list_max_entries, 2048);
        assert_eq!(config.list_all_timeout, std::time::Duration::from_secs(300));
        assert_eq!(
            config.end_wait_timeout,
            std::time::Duration::from_millis(500)
        );

        let yaml = yaml_doc!(
            r#"
                list_max_line_length: "2KB"
            "#
        );
        let config = FtpTransferConfig::parse_yaml(&yaml).unwrap();
        assert_eq!(config.list_max_line_len, 2000);
    }

    #[test]
    fn ftp_transfer_config_parse_err() {
        let yaml = yaml_doc!(
            r#"
                invalid_key: "value"
            "#
        );
        assert!(FtpTransferConfig::parse_yaml(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                list_max_line_len: "4KBX"
            "#
        );
        assert!(FtpTransferConfig::parse_yaml(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                list_max_entries: -2048
            "#
        );
        assert!(FtpTransferConfig::parse_yaml(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                list_all_timeout: "5x"
            "#
        );
        assert!(FtpTransferConfig::parse_yaml(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                end_wait_timeout: "-500ms"
            "#
        );
        assert!(FtpTransferConfig::parse_yaml(&yaml).is_err());

        let yaml = yaml_str!("invalid");
        assert!(FtpTransferConfig::parse_yaml(&yaml).is_err());
    }

    #[test]
    fn ftp_client_config_parse_ok() {
        let yaml = yaml_doc!(
            r#"
                control:
                  max_line_len: "2KB"
                  max_multi_lines: 128
                  command_timeout: "15s"
                transfer:
                  list_max_line_len: "4KB"
                  list_max_entries: 1024
                  list_all_timeout: "2m"
                  end_wait_timeout: "1s"
                connect_timeout: "10s"
                greeting_timeout: "5s"
                always_try_epsv: false
            "#
        );
        let config = FtpClientConfig::parse_yaml(&yaml).unwrap();
        assert_eq!(config.control.max_line_len, 2000);
        assert_eq!(config.control.max_multi_lines, 128);
        assert_eq!(
            config.control.command_timeout,
            std::time::Duration::from_secs(15)
        );
        assert_eq!(config.transfer.list_max_line_len, 4000);
        assert_eq!(config.transfer.list_max_entries, 1024);
        assert_eq!(
            config.transfer.list_all_timeout,
            std::time::Duration::from_secs(120)
        );
        assert_eq!(
            config.transfer.end_wait_timeout,
            std::time::Duration::from_secs(1)
        );
        assert_eq!(config.connect_timeout, std::time::Duration::from_secs(10));
        assert_eq!(config.greeting_timeout, std::time::Duration::from_secs(5));
        assert!(!config.always_try_epsv);
    }

    #[test]
    fn ftp_client_config_parse_err() {
        let yaml = yaml_doc!(
            r#"
                invalid_key: "value"
            "#
        );
        assert!(FtpClientConfig::parse_yaml(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                control: "invalid"
            "#
        );
        assert!(FtpClientConfig::parse_yaml(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                transfer: 1234
            "#
        );
        assert!(FtpClientConfig::parse_yaml(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                connect_timeout: "-10s"
            "#
        );
        assert!(FtpClientConfig::parse_yaml(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                greeting_timeout: "5z"
            "#
        );
        assert!(FtpClientConfig::parse_yaml(&yaml).is_err());

        let yaml = yaml_doc!(
            r#"
                always_try_epsv: "not_a_boolean"
            "#
        );
        assert!(FtpClientConfig::parse_yaml(&yaml).is_err());

        let yaml = yaml_str!("invalid");
        assert!(FtpClientConfig::parse_yaml(&yaml).is_err());
    }

    #[test]
    fn parse_invalid_yaml_types() {
        let yaml = Yaml::Array(vec![]);
        assert!(FtpControlConfig::parse_yaml(&yaml).is_err());
        assert!(FtpTransferConfig::parse_yaml(&yaml).is_err());
        assert!(FtpClientConfig::parse_yaml(&yaml).is_err());

        let yaml = Yaml::Integer(123);
        assert!(FtpControlConfig::parse_yaml(&yaml).is_err());
        assert!(FtpTransferConfig::parse_yaml(&yaml).is_err());
        assert!(FtpClientConfig::parse_yaml(&yaml).is_err());

        let yaml = Yaml::Boolean(true);
        assert!(FtpControlConfig::parse_yaml(&yaml).is_err());
        assert!(FtpTransferConfig::parse_yaml(&yaml).is_err());
        assert!(FtpClientConfig::parse_yaml(&yaml).is_err());

        let yaml = Yaml::Null;
        assert!(FtpControlConfig::parse_yaml(&yaml).is_err());
        assert!(FtpTransferConfig::parse_yaml(&yaml).is_err());
        assert!(FtpClientConfig::parse_yaml(&yaml).is_err());
    }
}
