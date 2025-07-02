/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::path::Path;
use std::str::FromStr;

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_types::net::{DnsEncryptionConfigBuilder, DnsEncryptionProtocol};

fn as_dns_encryption_protocol(value: &Yaml) -> anyhow::Result<DnsEncryptionProtocol> {
    if let Yaml::String(s) = value {
        DnsEncryptionProtocol::from_str(s).context("invalid dns encryption protocol value")
    } else {
        Err(anyhow!(
            "yaml type for 'dns encryption protocol' should be 'string'"
        ))
    }
}

pub fn as_dns_encryption_protocol_builder(
    value: &Yaml,
    lookup_dir: Option<&Path>,
) -> anyhow::Result<DnsEncryptionConfigBuilder> {
    const KEY_TLS_NAME: &str = "tls_name";

    match value {
        Yaml::Hash(map) => {
            let name_v = crate::hash_get_required(map, KEY_TLS_NAME)?;
            let name = crate::value::as_rustls_server_name(name_v).context(format!(
                "invalid tls server name value for key {KEY_TLS_NAME}",
            ))?;

            let mut config = DnsEncryptionConfigBuilder::new(name);
            crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
                KEY_TLS_NAME => Ok(()),
                "protocol" => {
                    let protocol = as_dns_encryption_protocol(v)
                        .context(format!("invalid dns encryption protocol value for key {k}"))?;
                    config.set_protocol(protocol);
                    Ok(())
                }
                "tls_client" => {
                    let builder = crate::value::as_rustls_client_config_builder(v, lookup_dir)
                        .context(format!("invalid tls client config value for key {k}"))?;
                    config.set_tls_client_config(builder);
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;

            Ok(config)
        }
        Yaml::String(_) => {
            let name = crate::value::as_rustls_server_name(value)
                .context("the string type value should be valid tls server name")?;
            Ok(DnsEncryptionConfigBuilder::new(name))
        }
        _ => Err(anyhow!("invalid value type")),
    }
}

#[cfg(test)]
#[cfg(feature = "rustls")]
mod tests {
    use super::*;
    use std::env::temp_dir;
    use std::fs;
    use yaml_rust::YamlLoader;

    #[test]
    fn test_as_dns_encryption_protocol_builder() {
        assert_eq!(
            as_dns_encryption_protocol(&Yaml::String("tls".to_string())).unwrap(),
            DnsEncryptionProtocol::Tls
        );
        assert_eq!(
            as_dns_encryption_protocol(&Yaml::String("https".to_string())).unwrap(),
            DnsEncryptionProtocol::Https
        );
        assert!(as_dns_encryption_protocol(&Yaml::String("invalid".to_string())).is_err());
        assert!(as_dns_encryption_protocol(&Yaml::Boolean(true)).is_err());

        let yaml = YamlLoader::load_from_str("example.com").unwrap();
        let builder = as_dns_encryption_protocol_builder(&yaml[0], None).unwrap();
        assert_eq!(builder.protocol(), DnsEncryptionProtocol::Tls);

        let yaml = YamlLoader::load_from_str(
            r#"
            tls_name: "dns.example.com"
            protocol: "https"
        "#,
        )
        .unwrap();
        let builder = as_dns_encryption_protocol_builder(&yaml[0], None).unwrap();
        assert_eq!(builder.protocol(), DnsEncryptionProtocol::Https);

        let yaml = YamlLoader::load_from_str(
            r#"
            tls_name: "dns.example.com"
            protocol: "doh"
        "#,
        )
        .unwrap();
        let builder = as_dns_encryption_protocol_builder(&yaml[0], None).unwrap();
        assert_eq!(builder.protocol(), DnsEncryptionProtocol::Https);

        let yaml = YamlLoader::load_from_str(
            r#"
            protocol: "tls"
        "#,
        )
        .unwrap();
        assert!(as_dns_encryption_protocol_builder(&yaml[0], None).is_err());

        let yaml = YamlLoader::load_from_str(
            r#"
            tls_name: "example.com"
            invalid_field: "value"
        "#,
        )
        .unwrap();
        assert!(as_dns_encryption_protocol_builder(&yaml[0], None).is_err());

        let yaml = YamlLoader::load_from_str(
            r#"
            tls_name: "example.com"
            protocol: "invalid_protocol"
        "#,
        )
        .unwrap();
        assert!(as_dns_encryption_protocol_builder(&yaml[0], None).is_err());

        assert!(as_dns_encryption_protocol_builder(&Yaml::Boolean(false), None).is_err());
        assert!(as_dns_encryption_protocol_builder(&Yaml::Integer(42), None).is_err());

        // Invalid string values
        let yaml = Yaml::String("".to_string());
        assert!(as_dns_encryption_protocol_builder(&yaml, None).is_err());

        let yaml = Yaml::String("!@#$%^&*()".to_string());
        assert!(as_dns_encryption_protocol_builder(&yaml, None).is_err());

        // Invalid value type
        let yaml = Yaml::Null;
        assert!(as_dns_encryption_protocol_builder(&yaml, None).is_err());

        let temp_dir = temp_dir();
        let test_dir = temp_dir.join("g3_yaml_test");
        fs::create_dir_all(&test_dir).unwrap();
        let cert_path = test_dir.join("test_cert.pem");

        let cert_content = r#"-----BEGIN CERTIFICATE-----
MIIDHzCCAgegAwIBAgIUWcRGf6EVDGnVyfKik4b3B3h/e2AwDQYJKoZIhvcNAQEL
BQAwGTEXMBUGA1UEAwwOZG5zLmV4YW1wbGUuY29tMB4XDTI0MDUyMjA4MTg0NloX
DTM0MDUxOTA4MTg0NlowGTEXMBUGA1UEAwwOZG5zLmV4YW1wbGUuY29tMIIBIjAN
BgkqhkiG9w0BAQEFAAOCAQ8AMIIBCgKCAQEAy1eX6o7dSpuG2b/lWl4i8z2u7T0F
U4C5J5mU+pRTd85KR2gC5fA1iTRZkvkM2SWLdWY2buYjJey06qf/B6F7pL/+s7s/
9/bDY8rC8M2B6y419aZg7qYm8+E4qvrA0u0aY5u0u1E8wYpP7t6m3F8g3L2Z6uY4
t+p8Q+c2C8b8s7x3d8t/g3e5h7k6l+f9i/s3e4k/r7v+w8f+y2e6n/j9o8c7s6k3
k5i4l8a9i/c3e+p8q+f9k/r8v/s4e+n6u+b9w/r7j/u5f+b8o/q9x/w6e+f7i/v6
k/t5b+p9s/u3g+d8a+h+k+e+qQIDAQABo1MwUTAdBgNVHQ4EFgQUQ3j1y3v6s8r8
s+f9d3b7e6r5q+wwHwYDVR0jBBgwFoAUQ3j1y3v6s8r8s+f9d3b7e6r5q+wwDAYD
VR0TBAUwAwEB/zANBgkqhkiG9w0BAQsFAAOCAQEAdpGZ+r6f+e4k/u7v/q8y/s5e
+e6q/s9g+u8p/u5k/v7f/q9z/r8h/s6f+g5i/r8k/s4h/u6e+e8h/u5k/v7f/q9z
/r8h/s6f+g5i/r8k/s4h/u6e+e8h/u5k/v7f/q9z/r8h/s6f+g5i/r8k/s4h/u6e
+e8h/u5k/v7f/q9z/r8h/s6f+g5i/r8k/s4h/u6e+e8h/u5k/v7f/q9z/r8h/s6f
+g5i/r8k/s4h/u6e+e8h/u5k/v7f/q9z/r8h/s6f+g5i/r8k/s4h/u6e+e8h/u4=
-----END CERTIFICATE-----"#;
        fs::write(&cert_path, cert_content).unwrap();

        let yaml = YamlLoader::load_from_str(&format!(
            r#"
                tls_name: "example.com"
                tls_client:
                  certificate: "{}"
            "#,
            cert_path.file_name().unwrap().to_str().unwrap()
        ))
        .unwrap();
        let result = as_dns_encryption_protocol_builder(&yaml[0], Some(&test_dir));
        assert!(result.is_ok());

        let yaml = YamlLoader::load_from_str(
            r#"
            tls_name: "example.com"
            tls_client:
              certificate: "non_existent_file.pem"
        "#,
        )
        .unwrap();
        let result = as_dns_encryption_protocol_builder(&yaml[0], Some(&test_dir));
        assert!(result.is_err());

        let _ = fs::remove_dir_all(&test_dir);
    }
}
