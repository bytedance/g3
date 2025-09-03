/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use super::{Facility, SyslogBackendBuilder, SyslogBuilder, SyslogFormatterKind};

impl SyslogBuilder {
    pub fn parse_yaml(value: &Yaml, ident: &'static str) -> anyhow::Result<Self> {
        match value {
            Yaml::Hash(map) => {
                let mut builder = SyslogBuilder::with_ident(ident);
                let mut use_cee_log_syntax = false;
                let mut cee_event_flag: Option<String> = None;
                builder.set_facility(Facility::Daemon);
                g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                    #[cfg(unix)]
                    "target_unix" | "backend_unix" => {
                        let backend = SyslogBackendBuilder::parse_unix_yaml(v)
                            .context(format!("invalid value for key {k}"))?;
                        builder.set_backend(backend);
                        Ok(())
                    }
                    "target_udp" | "backend_udp" => {
                        let backend = SyslogBackendBuilder::parse_udp_yaml(v)
                            .context(format!("invalid value for key {k}"))?;
                        builder.set_backend(backend);
                        Ok(())
                    }
                    "target" | "backend" => {
                        if let Yaml::Hash(map) = v {
                            g3_yaml::foreach_kv(map, |k, v| {
                                match g3_yaml::key::normalize(k).as_str() {
                                    "udp" => {
                                        let backend = SyslogBackendBuilder::parse_udp_yaml(v)
                                            .context(format!("invalid value for key {k}"))?;
                                        builder.set_backend(backend);
                                        Ok(())
                                    }
                                    #[cfg(unix)]
                                    "unix" => {
                                        let backend = SyslogBackendBuilder::parse_unix_yaml(v)
                                            .context(format!("invalid value for key {k}"))?;
                                        builder.set_backend(backend);
                                        Ok(())
                                    }
                                    _ => Err(anyhow!("invalid key {k}")),
                                }
                            })
                            .context(format!("invalid value for key {k}"))
                        } else {
                            Err(anyhow!("yaml value type for key {k} should be 'map'"))
                        }
                    }
                    "format_rfc5424" => {
                        let format = SyslogFormatterKind::parse_rfc5424_yaml(v)
                            .context(format!("invalid value for key {k}"))?;
                        builder.set_format(format);
                        Ok(())
                    }
                    "use_cee_log_syntax" | "use_cls" => {
                        use_cee_log_syntax = g3_yaml::value::as_bool(v)
                            .context(format!("invalid boolean value for key {k}"))?;
                        Ok(())
                    }
                    "cee_event_flag" | "cee_cookie" => {
                        let s = g3_yaml::value::as_ascii(v)
                            .context(format!("invalid ascii string value for key {k}"))?;
                        cee_event_flag = Some(s.to_string());
                        Ok(())
                    }
                    "emit_hostname" => {
                        let enable = g3_yaml::value::as_bool(v)
                            .context(format!("invalid boolean value for key {k}"))?;
                        builder.set_emit_hostname(enable);
                        Ok(())
                    }
                    "append_report_ts" => {
                        let enable = g3_yaml::value::as_bool(v)
                            .context(format!("invalid boolean value for key {k}"))?;
                        builder.append_report_ts(enable);
                        Ok(())
                    }
                    _ => Err(anyhow!("invalid key {k}")),
                })?;
                if use_cee_log_syntax {
                    builder.enable_cee_log_syntax(cee_event_flag);
                }
                Ok(builder)
            }
            Yaml::Null => {
                let mut builder = SyslogBuilder::with_ident(ident);
                builder.set_facility(Facility::Daemon);
                Ok(builder)
            }
            _ => Err(anyhow!(
                "yaml value type for 'SyslogBuilder' should be 'map'"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g3_yaml::yaml_doc;
    use yaml_rust::YamlLoader;

    #[test]
    fn parse_yaml_null() {
        let builder = SyslogBuilder::parse_yaml(&Yaml::Null, "test").unwrap();
        assert_eq!(builder.ident, "test");
        assert!(matches!(builder.facility, Facility::Daemon));
    }

    #[test]
    fn parse_yaml_invalid_types() {
        assert!(SyslogBuilder::parse_yaml(&Yaml::Array(vec![]), "test").is_err());

        assert!(SyslogBuilder::parse_yaml(&Yaml::Integer(123), "test").is_err());

        assert!(SyslogBuilder::parse_yaml(&Yaml::Real("1.23".to_string()), "test").is_err());
    }

    #[test]
    fn parse_yaml_udp_backend() {
        let yaml = yaml_doc!(
            r#"
            target_udp:
              address: "192.168.1.2:514"
              bind_ip: "192.168.1.1"
            format_rfc5424:
              enterprise_id: 32473
              message_id: "APP01"
            emit_hostname: true
            append_report_ts: true
            "#
        );
        let builder = SyslogBuilder::parse_yaml(&yaml, "test").unwrap();
        match &builder.backend {
            SyslogBackendBuilder::Udp(bind, addr) => {
                assert_eq!(addr, &"192.168.1.2:514".parse().unwrap());
                assert_eq!(bind, &Some("192.168.1.1".parse().unwrap()));
            }
            _ => panic!("Expected UDP backend"),
        }
        match &builder.format {
            SyslogFormatterKind::Rfc5424(eid, mid) => {
                assert_eq!(*eid, 32473);
                assert_eq!(mid, &Some("APP01".to_string()));
            }
            _ => panic!("Expected Rfc5424 formatter"),
        }
        assert!(builder.emit_hostname);
        assert!(builder.append_report_ts);

        let yaml = yaml_doc!(
            r#"
            target:
              udp:
                addr: "10.0.0.2:514"
                bind: "10.0.0.1"
            format_rfc5424: "MYAPP"
            use_cee_log_syntax: true
            cee_event_flag: "custom_flag"
            "#
        );
        let builder = SyslogBuilder::parse_yaml(&yaml, "test").unwrap();
        match &builder.backend {
            SyslogBackendBuilder::Udp(bind, addr) => {
                assert_eq!(addr, &"10.0.0.2:514".parse().unwrap());
                assert_eq!(bind, &Some("10.0.0.1".parse().unwrap()));
            }
            _ => panic!("Expected UDP backend"),
        }
        match &builder.format {
            SyslogFormatterKind::Rfc5424Cee(mid, flag) => {
                assert_eq!(flag, "custom_flag");
                assert_eq!(mid, &Some("MYAPP".to_string()));
            }
            _ => panic!("Expected Rfc5424Cee formatter"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn parse_yaml_unix_backend() {
        let yaml = yaml_doc!(
            r#"
            target_unix:
              path: "/dev/log"
            use_cls: true
            cee_cookie: "alt_flag"
            emit_hostname: false
            append_report_ts: false
            "#
        );
        let builder = SyslogBuilder::parse_yaml(&yaml, "test").unwrap();
        match &builder.backend {
            SyslogBackendBuilder::Unix(path) => {
                assert_eq!(path, &Some(std::path::PathBuf::from("/dev/log")));
            }
            _ => panic!("Expected Unix backend"),
        }
        match &builder.format {
            SyslogFormatterKind::Rfc3164Cee(flag) => {
                assert_eq!(flag, "alt_flag");
            }
            _ => panic!("Expected Rfc3164Cee formatter"),
        }
        assert!(!builder.emit_hostname);
        assert!(!builder.append_report_ts);

        let yaml = yaml_doc!(
            r#"
            backend:
              unix: "/dev/log"
            format_rfc5424: 12345
            "#
        );
        let builder = SyslogBuilder::parse_yaml(&yaml, "test").unwrap();
        match &builder.backend {
            SyslogBackendBuilder::Unix(path) => {
                assert_eq!(path, &Some(std::path::PathBuf::from("/dev/log")));
            }
            _ => panic!("Expected Unix backend"),
        }
        match &builder.format {
            SyslogFormatterKind::Rfc5424(eid, mid) => {
                assert_eq!(*eid, 12345);
                assert_eq!(mid, &None);
            }
            _ => panic!("Expected Rfc5424 formatter"),
        }
    }

    #[test]
    fn parse_yaml_err() {
        let yanl = yaml_doc!(
            r#"
            target_udp:
              invalid_key: "value"
            "#
        );
        assert!(SyslogBuilder::parse_yaml(&yanl, "test").is_err());

        let yaml = yaml_doc!(
            r#"
            backend_udp:
              bind_ip: "192.168.1.1"
            "#
        );
        assert!(SyslogBuilder::parse_yaml(&yaml, "test").is_err());

        let yaml = yaml_doc!(
            r#"
            target_udp: "invalid_address"
            "#
        );
        assert!(SyslogBuilder::parse_yaml(&yaml, "test").is_err());

        let yaml = yaml_doc!(
            r#"
            backend_udp: 12345
            "#
        );
        assert!(SyslogBuilder::parse_yaml(&yaml, "test").is_err());

        #[cfg(unix)]
        {
            let yaml = yaml_doc!(
                r#"
                target_unix:
                  invalid_key: "value"
                "#
            );
            assert!(SyslogBuilder::parse_yaml(&yaml, "test").is_err());

            let yaml = yaml_doc!(
                r#"
                backend_unix:
                  path: None
                "#
            );
            assert!(SyslogBuilder::parse_yaml(&yaml, "test").is_err());

            let yaml = yaml_doc!(
                r#"
                target_unix: "invalid_path"
                "#
            );
            assert!(SyslogBuilder::parse_yaml(&yaml, "test").is_err());

            let yaml = yaml_doc!(
                r#"
                backend_unix: 67890
                "#
            );
            assert!(SyslogBuilder::parse_yaml(&yaml, "test").is_err());
        }

        let yaml = yaml_doc!(
            r#"
            target:
                invalid: "value"
            "#
        );
        assert!(SyslogBuilder::parse_yaml(&yaml, "test").is_err());

        let yaml = yaml_doc!(
            r#"
            backend: "invalid"
            "#
        );
        assert!(SyslogBuilder::parse_yaml(&yaml, "test").is_err());

        let yaml = yaml_doc!(
            r#"
            format_rfc5424:
              invalid_key: "value"
            "#
        );
        assert!(SyslogBuilder::parse_yaml(&yaml, "test").is_err());

        let yaml = yaml_doc!(
            r#"
            format_rfc5424: false
            "#
        );
        assert!(SyslogBuilder::parse_yaml(&yaml, "test").is_err());

        let yaml = yaml_doc!(
            r#"
            use_cee_log_syntax: "not_bool"
            "#
        );
        assert!(SyslogBuilder::parse_yaml(&yaml, "test").is_err());

        let yaml = yaml_doc!(
            r#"
            cee_event_flag: 标志
            "#
        );
        assert!(SyslogBuilder::parse_yaml(&yaml, "test").is_err());

        let yaml = yaml_doc!(
            r#"
            emit_hostname: "not_bool"
            "#
        );
        assert!(SyslogBuilder::parse_yaml(&yaml, "test").is_err());

        let yaml = yaml_doc!(
            r#"
            append_report_ts: "not_bool"
            "#
        );
        assert!(SyslogBuilder::parse_yaml(&yaml, "test").is_err());

        let yaml = yaml_doc!(
            r#"
            invalid_key: "value"
            "#
        );
        assert!(SyslogBuilder::parse_yaml(&yaml, "test").is_err());
    }
}
