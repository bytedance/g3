/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use rustls_pki_types::pem::PemObject;
use rustls_pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use serde_json::Value;

use g3_types::net::{
    RustlsCertificatePair, RustlsCertificatePairBuilder, RustlsClientConfigBuilder,
    RustlsServerConfigBuilder,
};

pub fn as_rustls_server_name(value: &Value) -> anyhow::Result<ServerName<'static>> {
    if let Value::String(s) = value {
        ServerName::try_from(s.as_str())
            .map(|r| r.to_owned())
            .map_err(|e| anyhow!("invalid rustls server name string: {e}"))
    } else {
        Err(anyhow!(
            "json value type for 'rustls server name' should be 'string'"
        ))
    }
}

fn as_certificates_from_single_element(
    value: &Value,
) -> anyhow::Result<Vec<CertificateDer<'static>>> {
    if let Value::String(s) = value {
        let mut certs = Vec::new();
        for (i, r) in CertificateDer::pem_slice_iter(s.as_bytes()).enumerate() {
            let cert = r.map_err(|e| anyhow!("invalid certificate #{i}: {e:?}"))?;
            certs.push(cert);
        }
        if certs.is_empty() {
            Err(anyhow!("no valid certificate found"))
        } else {
            Ok(certs)
        }
    } else {
        Err(anyhow!("json value type 'certificates' should be 'string'"))
    }
}

pub fn as_rustls_certificates(value: &Value) -> anyhow::Result<Vec<CertificateDer<'static>>> {
    if let Value::Array(seq) = value {
        let mut certs = Vec::new();
        for (i, v) in seq.iter().enumerate() {
            let this_certs = as_certificates_from_single_element(v)
                .context(format!("invalid certificates value for element #{i}"))?;
            certs.extend(this_certs);
        }
        Ok(certs)
    } else {
        as_certificates_from_single_element(value)
    }
}

pub fn as_rustls_private_key(value: &Value) -> anyhow::Result<PrivateKeyDer<'static>> {
    if let Value::String(s) = value {
        PrivateKeyDer::from_pem_slice(s.as_bytes())
            .map_err(|e| anyhow!("invalid private key string: {e:?}"))
    } else {
        Err(anyhow!(
            "json value type for 'private key' should be 'string'"
        ))
    }
}

pub fn as_rustls_certificate_pair(value: &Value) -> anyhow::Result<RustlsCertificatePair> {
    if let Value::Object(map) = value {
        let mut pair_builder = RustlsCertificatePairBuilder::default();
        for (k, v) in map {
            match crate::key::normalize(k).as_str() {
                "certificate" | "cert" => {
                    let certs = as_rustls_certificates(v)
                        .context(format!("invalid certificates value for key {k}"))?;
                    pair_builder.set_certs(certs);
                }
                "private_key" | "key" => {
                    let key = as_rustls_private_key(v)
                        .context(format!("invalid private key value for key {k}"))?;
                    pair_builder.set_key(key);
                }
                _ => return Err(anyhow!("invalid key {k}")),
            }
        }
        pair_builder.build()
    } else {
        Err(anyhow!(
            "json value type for rustls certificate pair should be 'map'"
        ))
    }
}

pub fn as_rustls_client_config_builder(value: &Value) -> anyhow::Result<RustlsClientConfigBuilder> {
    if let Value::Object(map) = value {
        let mut builder = RustlsClientConfigBuilder::default();
        let mut cert_pair_builder = RustlsCertificatePairBuilder::default();

        for (k, v) in map {
            match crate::key::normalize(k).as_str() {
                "no_session_cache" | "disable_session_cache" | "session_cache_disabled" => {
                    let no = crate::value::as_bool(v)
                        .context(format!("invalid bool value for key {k}"))?;
                    if no {
                        builder.set_no_session_cache();
                    }
                }
                "disable_sni" => {
                    let disable = crate::value::as_bool(v)
                        .context(format!("invalid bool value for key {k}"))?;
                    if disable {
                        builder.set_disable_sni();
                    }
                }
                "max_fragment_size" => {
                    let mtu = crate::value::as_usize(v)
                        .context(format!("invalid usize value for key {k}"))?;
                    builder.set_max_fragment_size(mtu);
                }
                "certificate" | "cert" => {
                    let certs = as_rustls_certificates(v)
                        .context(format!("invalid certificates value for key {k}"))?;
                    cert_pair_builder.set_certs(certs);
                }
                "private_key" | "key" => {
                    let key = as_rustls_private_key(v)
                        .context(format!("invalid private key value for key {k}"))?;
                    cert_pair_builder.set_key(key);
                }
                "cert_pair" => {
                    let pair = as_rustls_certificate_pair(v)
                        .context(format!("invalid cert pair value for key {k}"))?;
                    builder.set_cert_pair(pair);
                }
                "ca_certificate" | "ca_cert" | "server_auth_certificate" | "server_auth_cert" => {
                    let certs = as_rustls_certificates(v)
                        .context(format!("invalid certificates value for key {k}"))?;
                    builder.set_ca_certificates(certs);
                }
                "no_default_ca_certificate" | "no_default_ca_cert" => {
                    let no_default = crate::value::as_bool(v)
                        .context(format!("invalid bool value for key {k}"))?;
                    if no_default {
                        builder.set_no_default_ca_certificates();
                    }
                }
                "use_builtin_ca_certificate" | "use_builtin_ca_cert" => {
                    let use_builtin = crate::value::as_bool(v)
                        .context(format!("invalid bool value for key {k}"))?;
                    if use_builtin {
                        builder.set_use_builtin_ca_certificates();
                    }
                }
                "handshake_timeout" | "negotiation_timeout" => {
                    let timeout = crate::humanize::as_duration(v)
                        .context(format!("invalid humanize duration value for key {k}"))?;
                    builder.set_negotiation_timeout(timeout);
                }
                _ => return Err(anyhow!("invalid key {k}")),
            }
        }

        if let Ok(cert_pair) = cert_pair_builder.build()
            && builder.set_cert_pair(cert_pair).is_some()
        {
            return Err(anyhow!("found duplicate client certificate config"));
        }

        builder.check()?;
        Ok(builder)
    } else {
        Err(anyhow!(
            "json value type for 'rustls client config builder' should be 'map'"
        ))
    }
}

pub fn as_rustls_server_config_builder(value: &Value) -> anyhow::Result<RustlsServerConfigBuilder> {
    if let Value::Object(map) = value {
        let mut builder = RustlsServerConfigBuilder::empty();
        let mut cert_pair_builder = RustlsCertificatePairBuilder::default();

        for (k, v) in map {
            match crate::key::normalize(k).as_str() {
                "cert_pairs" => {
                    if let Value::Array(seq) = value {
                        for (i, v) in seq.iter().enumerate() {
                            let pair = as_rustls_certificate_pair(v)
                                .context(format!("invalid rustls cert pair value for {k}#{i}"))?;
                            builder.push_cert_pair(pair);
                        }
                    } else {
                        let pair = as_rustls_certificate_pair(v)
                            .context(format!("invalid rustls cert pair value for key {k}"))?;
                        builder.push_cert_pair(pair);
                    }
                }
                "certificate" | "cert" => {
                    let certs =
                        as_rustls_certificates(v).context(format!("invalid value for key {k}"))?;
                    cert_pair_builder.set_certs(certs);
                }
                "private_key" | "key" => {
                    let key =
                        as_rustls_private_key(v).context(format!("invalid value for key {k}"))?;
                    cert_pair_builder.set_key(key);
                }
                "enable_client_auth" => {
                    let enable =
                        crate::value::as_bool(v).context(format!("invalid value for key {k}"))?;
                    if enable {
                        builder.enable_client_auth();
                    }
                }
                "use_session_ticket" => {
                    let enable =
                        crate::value::as_bool(v).context(format!("invalid value for key {k}"))?;
                    builder.set_use_session_ticket(enable);
                }
                "no_session_ticket" | "disable_session_ticket" => {
                    let disable =
                        crate::value::as_bool(v).context(format!("invalid value for key {k}"))?;
                    builder.set_disable_session_ticket(disable);
                }
                "no_session_cache" | "disable_session_cache" => {
                    let disable =
                        crate::value::as_bool(v).context(format!("invalid value for key {k}"))?;
                    builder.set_disable_session_cache(disable);
                }
                "ca_certificate" | "ca_cert" | "client_auth_certificate" | "client_auth_cert" => {
                    let certs =
                        as_rustls_certificates(v).context(format!("invalid value for key {k}"))?;
                    builder.set_client_auth_certificates(certs);
                }
                "handshake_timeout" | "negotiation_timeout" | "accept_timeout" => {
                    let timeout = crate::humanize::as_duration(v)
                        .context(format!("invalid humanize duration value for key {k}"))?;
                    builder.set_accept_timeout(timeout);
                }
                _ => return Err(anyhow!("invalid key {k}")),
            }
        }

        if let Ok(cert_pair) = cert_pair_builder.build() {
            builder.push_cert_pair(cert_pair);
        }

        builder.check()?;
        Ok(builder)
    } else {
        Err(anyhow!(
            "json value type for 'rustls server config builder' should be 'map'"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::time::Duration;

    const TEST_CERT1_PEM: &str = include_str!("test_data/test_cert1.pem");
    const TEST_CERT2_PEM: &str = include_str!("test_data/test_cert2.pem");
    const TEST_KEY1_PEM: &str = include_str!("test_data/test_key1.pem");

    #[test]
    fn as_rustls_server_name_ok() {
        // Valid DNS name
        let value = json!("example.com");
        assert_eq!(
            as_rustls_server_name(&value).unwrap(),
            ServerName::try_from("example.com").unwrap()
        );

        // Valid IP address
        let value = json!("192.168.0.1");
        assert_eq!(
            as_rustls_server_name(&value).unwrap(),
            ServerName::try_from("192.168.0.1").unwrap()
        );
    }

    #[test]
    fn as_rustls_server_name_err() {
        // Invalid domain
        let value = json!("invalid domain");
        assert!(as_rustls_server_name(&value).is_err());

        // Non-string type
        let value = json!(123);
        assert!(as_rustls_server_name(&value).is_err());
    }

    #[test]
    fn as_rustls_certificates_ok() {
        // Single PEM string
        let value = json!(TEST_CERT1_PEM);
        let certs = as_rustls_certificates(&value).unwrap();
        assert!(!certs.is_empty());

        // Array of PEM strings
        let value = json!([TEST_CERT1_PEM, TEST_CERT2_PEM]);
        let certs = as_rustls_certificates(&value).unwrap();
        assert_eq!(certs.len(), 2);
    }

    #[test]
    fn as_rustls_certificates_err() {
        // Invalid PEM
        let value = json!("invalid");
        assert!(as_rustls_certificates(&value).is_err());

        // Empty array
        let value = json!([""]);
        assert!(as_rustls_certificates(&value).is_err());

        // Non-string element in array
        let value = json!([123]);
        assert!(as_rustls_certificates(&value).is_err());
    }

    #[test]
    fn as_rustls_private_key_ok() {
        // Valid private key
        let value = json!(TEST_KEY1_PEM);
        let key = as_rustls_private_key(&value).unwrap();
        assert!(matches!(key, PrivateKeyDer::Pkcs8(_)));
    }

    #[test]
    fn as_rustls_private_key_err() {
        // Invalid private key
        let value = json!("invalid");
        assert!(as_rustls_private_key(&value).is_err());

        // Non-string type
        let value = json!(123);
        assert!(as_rustls_private_key(&value).is_err());
    }

    #[test]
    fn as_rustls_certificate_pair_ok() {
        // Valid certificate pair
        let value = json!({
            "certificate": TEST_CERT1_PEM,
            "private_key": TEST_KEY1_PEM
        });
        let pair = as_rustls_certificate_pair(&value).unwrap();
        let mut builder = RustlsCertificatePairBuilder::default();
        builder.set_certs(as_certificates_from_single_element(&json!(TEST_CERT1_PEM)).unwrap());
        builder.set_key(as_rustls_private_key(&json!(TEST_KEY1_PEM)).unwrap());
        let expected = builder.build().unwrap();
        assert_eq!(pair.certs_owned(), expected.certs_owned());
    }

    #[test]
    fn as_rustls_certificate_pair_err() {
        // Missing certificate
        let value = json!({"private_key": TEST_KEY1_PEM});
        assert!(as_rustls_certificate_pair(&value).is_err());

        // Missing private key
        let value = json!({"certificate": TEST_CERT1_PEM});
        assert!(as_rustls_certificate_pair(&value).is_err());

        // Invalid key
        let value = json!({"invalid_key": "value"});
        assert!(as_rustls_certificate_pair(&value).is_err());

        // Non-object type
        let value = json!("invalid");
        assert!(as_rustls_certificate_pair(&value).is_err());
    }

    #[test]
    fn as_rustls_client_config_builder_ok() {
        // Full config
        let value = json!({
            "no_session_cache": true,
            "disable_sni": true,
            "max_fragment_size": 1400,
            "certificate": TEST_CERT1_PEM,
            "private_key": TEST_KEY1_PEM,
            "ca_certificate": TEST_CERT1_PEM,
            "no_default_ca_certificate": true,
            "use_builtin_ca_certificate": true,
            "handshake_timeout": "10s"
        });
        let builder = as_rustls_client_config_builder(&value).unwrap();
        let mut expected = RustlsClientConfigBuilder::default();
        expected.set_no_session_cache();
        expected.set_disable_sni();
        expected.set_max_fragment_size(1400);
        let mut pair_builder = RustlsCertificatePairBuilder::default();
        pair_builder.set_certs(as_rustls_certificates(&json!(TEST_CERT1_PEM)).unwrap());
        pair_builder.set_key(as_rustls_private_key(&json!(TEST_KEY1_PEM)).unwrap());
        expected.set_cert_pair(pair_builder.build().unwrap());
        expected.set_ca_certificates(as_rustls_certificates(&json!(TEST_CERT1_PEM)).unwrap());
        expected.set_no_default_ca_certificates();
        expected.set_use_builtin_ca_certificates();
        expected.set_negotiation_timeout(Duration::from_secs(10));
        assert_eq!(builder, expected);

        // Cert_pair config
        let value = json!({
            "cert_pair": {
                "certificate": TEST_CERT1_PEM,
                "private_key": TEST_KEY1_PEM
            }
        });
        let builder = as_rustls_client_config_builder(&value).unwrap();
        let mut expected = RustlsClientConfigBuilder::default();
        let mut pair_builder = RustlsCertificatePairBuilder::default();
        pair_builder.set_certs(as_rustls_certificates(&json!(TEST_CERT1_PEM)).unwrap());
        pair_builder.set_key(as_rustls_private_key(&json!(TEST_KEY1_PEM)).unwrap());
        expected.set_cert_pair(pair_builder.build().unwrap());
        assert_eq!(builder, expected);
    }

    #[test]
    fn as_rustls_client_config_builder_err() {
        // Duplicate certificate config
        let value = json!({
            "cert": TEST_CERT1_PEM,
            "key": TEST_KEY1_PEM,
            "cert_pair": {
                "certificate": TEST_CERT1_PEM,
                "private_key": TEST_KEY1_PEM
            }
        });
        assert!(as_rustls_client_config_builder(&value).is_err());

        // Invalid key
        let value = json!({
            "invalid_key": "value"
        });
        assert!(as_rustls_client_config_builder(&value).is_err());

        // Invalid value type
        let value = json!(123);
        assert!(as_rustls_client_config_builder(&value).is_err());
    }

    #[test]
    fn as_rustls_server_config_builder_ok() {
        // Full config
        let value = json!({
            "cert_pairs": {
                    "certificate": TEST_CERT1_PEM,
                    "private_key": TEST_KEY1_PEM
            },
            "enable_client_auth": true,
            "use_session_ticket": false,
            "no_session_cache": true,
            "ca_certificate": TEST_CERT1_PEM,
            "handshake_timeout": "10s"
        });
        let builder = as_rustls_server_config_builder(&value).unwrap();
        let mut expected = RustlsServerConfigBuilder::empty();
        let mut pair_builder = RustlsCertificatePairBuilder::default();
        pair_builder.set_certs(as_rustls_certificates(&json!(TEST_CERT1_PEM)).unwrap());
        pair_builder.set_key(as_rustls_private_key(&json!(TEST_KEY1_PEM)).unwrap());
        expected.push_cert_pair(pair_builder.build().unwrap());
        expected.enable_client_auth();
        expected.set_use_session_ticket(false);
        expected.set_disable_session_cache(true);
        expected
            .set_client_auth_certificates(as_rustls_certificates(&json!(TEST_CERT1_PEM)).unwrap());
        expected.set_accept_timeout(Duration::from_secs(10));
        assert_eq!(builder, expected);

        // Certificate/key fields
        let value = json!({
            "certificate": TEST_CERT1_PEM,
            "private_key": TEST_KEY1_PEM,
            "no_session_ticket": true,
        });
        let builder = as_rustls_server_config_builder(&value).unwrap();
        let mut expected = RustlsServerConfigBuilder::empty();
        let mut pair_builder = RustlsCertificatePairBuilder::default();
        pair_builder.set_certs(as_rustls_certificates(&json!(TEST_CERT1_PEM)).unwrap());
        pair_builder.set_key(as_rustls_private_key(&json!(TEST_KEY1_PEM)).unwrap());
        expected.push_cert_pair(pair_builder.build().unwrap());
        expected.set_disable_session_ticket(true);
        assert_eq!(builder, expected);
    }

    #[test]
    fn as_rustls_server_config_builder_err() {
        // Invalid key
        let value = json!({
            "invalid_key": "value"
        });
        assert!(as_rustls_server_config_builder(&value).is_err());

        // Invalid value type
        let value = json!("invalid");
        assert!(as_rustls_server_config_builder(&value).is_err());
    }
}
