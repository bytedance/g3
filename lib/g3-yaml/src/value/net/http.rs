/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::{Context, anyhow};
use http::uri::PathAndQuery;
use http::{HeaderName, HeaderValue};
use yaml_rust::Yaml;

use g3_types::net::{
    HttpForwardCapability, HttpForwardedHeaderType, HttpKeepAliveConfig, HttpServerId,
};

pub fn as_http_keepalive_config(v: &Yaml) -> anyhow::Result<HttpKeepAliveConfig> {
    let mut config = HttpKeepAliveConfig::default();

    match v {
        Yaml::Hash(map) => {
            crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
                "enable" => {
                    let enable = crate::value::as_bool(v)?;
                    config.set_enable(enable);
                    Ok(())
                }
                "idle_expire" => {
                    let idle_expire = crate::humanize::as_duration(v)?;
                    config.set_idle_expire(idle_expire);
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;
        }
        Yaml::Boolean(enable) => {
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

pub fn as_http_forwarded_header_type(value: &Yaml) -> anyhow::Result<HttpForwardedHeaderType> {
    match crate::value::as_bool(value) {
        Ok(true) => Ok(HttpForwardedHeaderType::default()),
        Ok(false) => Ok(HttpForwardedHeaderType::Disable),
        Err(_) => {
            if let Yaml::String(s) = value {
                HttpForwardedHeaderType::from_str(s)
                    .map_err(|_| anyhow!("invalid string value for 'HttpForwardedHeaderType'"))
            } else {
                Err(anyhow!(
                    "yaml value type for 'IpAddr' should be 'boolean' or 'string'"
                ))
            }
        }
    }
}

pub fn as_http_forward_capability(value: &Yaml) -> anyhow::Result<HttpForwardCapability> {
    let mut cap = HttpForwardCapability::default();

    match value {
        Yaml::Hash(map) => {
            if let Ok(v) = crate::hash_get_required(map, "forward_ftp") {
                let enable =
                    crate::value::as_bool(v).context("invalid bool value for key forward_ftp")?;
                cap.set_forward_ftp_all(enable);
            }

            crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
                "forward_https" => {
                    let enable = crate::value::as_bool(v)?;
                    cap.set_forward_https(enable);
                    Ok(())
                }
                "forward_ftp_get" => {
                    let enable = crate::value::as_bool(v)?;
                    cap.set_forward_ftp_get(enable);
                    Ok(())
                }
                "forward_ftp_put" => {
                    let enable = crate::value::as_bool(v)?;
                    cap.set_forward_ftp_put(enable);
                    Ok(())
                }
                "forward_ftp_del" => {
                    let enable = crate::value::as_bool(v)?;
                    cap.set_forward_ftp_del(enable);
                    Ok(())
                }
                "forward_ftp" => Ok(()),
                _ => Err(anyhow!("invalid key {k}")),
            })?;
        }
        _ => return Err(anyhow!("invalid yaml value type for HttpForwardCapability")),
    }

    Ok(cap)
}

pub fn as_http_server_id(value: &Yaml) -> anyhow::Result<HttpServerId> {
    if let Yaml::String(s) = value {
        let id = HttpServerId::from_str(s)?;
        Ok(id)
    } else {
        Err(anyhow!(
            "yaml value type for 'HttpServerId' should be 'string'"
        ))
    }
}

pub fn as_http_header_name(value: &Yaml) -> anyhow::Result<HeaderName> {
    if let Yaml::String(s) = value {
        HeaderName::from_str(s).map_err(|e| anyhow!(e))
    } else {
        Err(anyhow!(
            "yaml value type for 'HttpHeaderName' should be 'string'"
        ))
    }
}

pub fn as_http_header_value_string(value: &Yaml) -> anyhow::Result<String> {
    let s = crate::value::as_string(value).context("invalid yaml value for http header value")?;
    HeaderValue::from_str(&s).map_err(|e| anyhow!("invalid http header value string {s}: {e}"))?;
    Ok(s)
}

pub fn as_http_path_and_query(value: &Yaml) -> anyhow::Result<PathAndQuery> {
    if let Yaml::String(s) = value {
        PathAndQuery::from_str(s).map_err(|e| anyhow!(e))
    } else {
        Err(anyhow!(
            "yaml value type for 'HttpPathAndQuery' should be 'string'"
        ))
    }
}

#[cfg(test)]
#[cfg(feature = "http")]
mod tests {
    use super::*;
    use http::Method;
    use std::time::Duration;
    use yaml_rust::YamlLoader;

    #[test]
    fn as_http_keepalive_config_ok() {
        // Valid config with enable and idle_expire
        let yaml = yaml_doc!(
            r#"
                enable: true
                idle_expire: 30s
            "#
        );
        let config = as_http_keepalive_config(&yaml).unwrap();
        assert!(config.is_enabled());
        assert_eq!(config.idle_expire(), Duration::from_secs(30));

        // Valid config with only enable
        let yaml = yaml_doc!(
            r#"
                enable: false
            "#
        );
        let config = as_http_keepalive_config(&yaml).unwrap();
        assert!(!config.is_enabled());
        assert_eq!(config.idle_expire(), Duration::from_nanos(0));

        // Valid config with only idle_expire
        let yaml = yaml_doc!(
            r#"
                idle_expire: 30s
            "#
        );
        let config = as_http_keepalive_config(&yaml).unwrap();
        assert!(config.is_enabled());
        assert_eq!(config.idle_expire(), Duration::from_secs(30));

        // Valid config with boolean value
        let yaml = Yaml::Boolean(true);
        let config = as_http_keepalive_config(&yaml).unwrap();
        assert!(config.is_enabled());

        // Valid config with string value
        let yaml = yaml_str!("30s");
        let config = as_http_keepalive_config(&yaml).unwrap();
        assert!(config.is_enabled());
        assert_eq!(config.idle_expire(), Duration::from_secs(30));

        // Valid config with integer value
        let yaml = Yaml::Integer(60);
        let config = as_http_keepalive_config(&yaml).unwrap();
        assert!(config.is_enabled());
        assert_eq!(config.idle_expire(), Duration::from_secs(60));
    }

    #[test]
    fn as_http_keepalive_config_err() {
        // Invalid config with wrong enable type
        let yaml = yaml_doc!(
            r#"
                enable: not_a_bool
            "#
        );
        assert!(as_http_keepalive_config(&yaml).is_err());

        // Invalid config with wrong idle_expire type
        let yaml = yaml_doc!(
            r#"
                idle_expire: not_a_duration
            "#
        );
        assert!(as_http_keepalive_config(&yaml).is_err());

        // Invalid config with wrong key
        let yaml = yaml_doc!(
            r#"
                invalid_key: true
            "#
        );
        assert!(as_http_keepalive_config(&yaml).is_err());

        // Invalid config with unsupported type
        let yaml = Yaml::Real("not_a_duration".to_string());
        assert!(as_http_keepalive_config(&yaml).is_err());
    }

    #[test]
    fn as_http_forwarded_header_type_ok() {
        // Valid config with boolean value
        let yaml = Yaml::Boolean(true);
        let header_type = as_http_forwarded_header_type(&yaml).unwrap();
        assert_eq!(header_type, HttpForwardedHeaderType::default());

        let yaml = Yaml::Boolean(false);
        let header_type = as_http_forwarded_header_type(&yaml).unwrap();
        assert_eq!(header_type, HttpForwardedHeaderType::Disable);

        // Valid config with string value
        let yaml = yaml_str!("Standard");
        let header_type = as_http_forwarded_header_type(&yaml).unwrap();
        assert_eq!(header_type, HttpForwardedHeaderType::Standard);

        // Valid config with integer value
        let yaml = Yaml::Integer(1);
        let header_type = as_http_forwarded_header_type(&yaml).unwrap();
        assert_eq!(header_type, HttpForwardedHeaderType::default());

        let yaml = Yaml::Integer(0);
        let header_type = as_http_forwarded_header_type(&yaml).unwrap();
        assert_eq!(header_type, HttpForwardedHeaderType::Disable);
    }

    #[test]
    fn as_http_forwarded_header_type_err() {
        // Invalid config with unsupported type
        let yaml = Yaml::Null;
        assert!(as_http_forwarded_header_type(&yaml).is_err());

        // Invalid config with invalid string value
        let yaml = yaml_str!("Invalid");
        assert!(as_http_forwarded_header_type(&yaml).is_err());
    }

    #[test]
    fn as_http_forward_capability_ok() {
        // Valid config with all forward options enabled
        let yaml = yaml_doc!(
            r#"
                forward_https: true
                forward_ftp: true
                forward_ftp_get: true
                forward_ftp_put: true
                forward_ftp_del: true
            "#
        );
        let cap = as_http_forward_capability(&yaml).unwrap();
        assert!(cap.forward_https());
        assert!(cap.forward_ftp(&Method::GET));
        assert!(cap.forward_ftp(&Method::PUT));
        assert!(cap.forward_ftp(&Method::DELETE));

        // Valid config with only HTTPS forwarding enabled
        let yaml = yaml_doc!("{ forward_https: true }");
        let cap = as_http_forward_capability(&yaml).unwrap();
        assert!(cap.forward_https());

        // Valid config with FTP forwarding enabled
        let yaml = yaml_doc!("{ forward_ftp: true }");
        let cap = as_http_forward_capability(&yaml).unwrap();
        assert!(cap.forward_ftp(&Method::GET));
        assert!(cap.forward_ftp(&Method::PUT));
        assert!(cap.forward_ftp(&Method::DELETE));

        let yaml = yaml_doc!(
            r#"
                forward_ftp_get: true
                forward_ftp_put: false
                forward_ftp_del: true
            "#
        );
        let cap = as_http_forward_capability(&yaml).unwrap();
        assert!(cap.forward_ftp(&Method::GET));
        assert!(!cap.forward_ftp(&Method::PUT));
        assert!(cap.forward_ftp(&Method::DELETE));

        // Valid config with only FTP GET forwarding enabled
        let yaml = yaml_doc!(
            r#"
                forward_ftp: false
                forward_ftp_get: true
            "#
        );
        let cap = as_http_forward_capability(&yaml).unwrap();
        assert!(cap.forward_ftp(&Method::GET));
        assert!(!cap.forward_ftp(&Method::PUT));
        assert!(!cap.forward_ftp(&Method::DELETE));
    }

    #[test]
    fn as_http_forward_capability_err() {
        // Invalid config with invalid key
        let yaml = yaml_doc!("{ invalid_key: true }");
        assert!(as_http_forward_capability(&yaml).is_err());

        // Invalid config with wrong value type
        let yaml = yaml_doc!("{ forward_https: not_a_bool }");
        assert!(as_http_forward_capability(&yaml).is_err());

        let yaml = yaml_doc!("{ forward_ftp: not_a_bool }");
        assert!(as_http_forward_capability(&yaml).is_err());

        let yaml = yaml_doc!("{ forward_ftp_get: not_a_bool }");
        assert!(as_http_forward_capability(&yaml).is_err());

        let yaml = yaml_doc!("{ forward_ftp_put: not_a_bool }");
        assert!(as_http_forward_capability(&yaml).is_err());

        let yaml = yaml_doc!("{ forward_ftp_del: not_a_bool }");
        assert!(as_http_forward_capability(&yaml).is_err());

        // Invalid config with unsupported type
        let yaml = Yaml::Null;
        assert!(as_http_forward_capability(&yaml).is_err());
    }

    #[test]
    fn as_http_server_id_ok() {
        // Valid config with string value
        let yaml = yaml_str!("server1");
        let id = as_http_server_id(&yaml).unwrap();
        assert_eq!(id.as_str(), "server1");
    }

    #[test]
    fn as_http_server_id_err() {
        // Invalid config with wrong value type
        let yaml = Yaml::Integer(123);
        assert!(as_http_server_id(&yaml).is_err());
    }

    #[test]
    fn as_http_header_name_ok() {
        // Valid header name
        let yaml = yaml_str!("Content-Type");
        let header_name = as_http_header_name(&yaml).unwrap();
        assert_eq!(header_name.as_str(), "content-type");
    }

    #[test]
    fn as_http_header_name_err() {
        // Invalid header name
        let yaml = yaml_str!("Invalid Header");
        assert!(as_http_header_name(&yaml).is_err());

        // Invalid type
        let yaml = Yaml::Integer(123);
        assert!(as_http_header_name(&yaml).is_err());
    }

    #[test]
    fn as_http_header_value_string_ok() {
        // Valid header value
        let yaml = yaml_str!("text/plain; charset=utf-8");
        let value = as_http_header_value_string(&yaml).unwrap();
        assert_eq!(value, "text/plain; charset=utf-8");

        let yaml = yaml_str!("");
        let value = as_http_header_value_string(&yaml).unwrap();
        assert_eq!(value, "");
    }

    #[test]
    fn as_http_header_value_string_err() {
        // Invalid header value
        let yaml = yaml_str!("Invalid\x0bValue");
        assert!(as_http_header_value_string(&yaml).is_err());

        // Invalid type
        let yaml = Yaml::Null;
        assert!(as_http_header_value_string(&yaml).is_err());
    }

    #[test]
    fn as_http_path_and_query_ok() {
        // Valid path and query
        let yaml = yaml_str!("/path?query=value");
        let path_and_query = as_http_path_and_query(&yaml).unwrap();
        assert_eq!(path_and_query.as_str(), "/path?query=value");
    }

    #[test]
    fn as_http_path_and_query_err() {
        // Invalid path and query
        let yaml = yaml_str!("Invalid Path");
        assert!(as_http_path_and_query(&yaml).is_err());

        // Invalid type
        let yaml = Yaml::Integer(123);
        assert!(as_http_path_and_query(&yaml).is_err());
    }
}
