/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::{Context, anyhow};
use openssl::pkey::{PKey, Private};
use openssl::x509::X509;
use serde_json::Value;

use g3_types::net::{
    OpensslCertificatePair, OpensslClientConfigBuilder, OpensslProtocol, OpensslTlcpCertificatePair,
};

fn as_certificates_from_single_element(value: &Value) -> anyhow::Result<Vec<X509>> {
    if let Value::String(s) = value {
        let certs = X509::stack_from_pem(s.as_bytes())
            .map_err(|e| anyhow!("invalid certificate string: {e}"))?;
        if certs.is_empty() {
            Err(anyhow!("no valid certificate found"))
        } else {
            Ok(certs)
        }
    } else {
        Err(anyhow!("json value type 'certificates' should be 'string'"))
    }
}

pub fn as_openssl_certificates(value: &Value) -> anyhow::Result<Vec<X509>> {
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

pub fn as_openssl_private_key(value: &Value) -> anyhow::Result<PKey<Private>> {
    if let Value::String(s) = value {
        PKey::private_key_from_pem(s.as_bytes())
            .map_err(|e| anyhow!("invalid private key string: {e}"))
    } else {
        Err(anyhow!(
            "json value type for 'private key' should be 'string'"
        ))
    }
}

pub fn as_openssl_certificate_pair(value: &Value) -> anyhow::Result<OpensslCertificatePair> {
    if let Value::Object(map) = value {
        let mut pair = OpensslCertificatePair::default();

        for (k, v) in map {
            match crate::key::normalize(k).as_str() {
                "certificate" | "cert" => {
                    let cert = as_openssl_certificates(v)
                        .context(format!("invalid certificates value for key {k}"))?;
                    pair.set_certificates(cert)
                        .context("failed to set certificate")?;
                }
                "private_key" | "key" => {
                    let key = as_openssl_private_key(v)
                        .context(format!("invalid private key value for key {k}"))?;
                    pair.set_private_key(key)
                        .context("failed to set private key")?;
                }
                _ => return Err(anyhow!("invalid key {k}")),
            }
        }

        pair.check()?;
        Ok(pair)
    } else {
        Err(anyhow!(
            "json value type for openssl certificate pair should be 'map'"
        ))
    }
}

pub fn as_openssl_tlcp_certificate_pair(
    value: &Value,
) -> anyhow::Result<OpensslTlcpCertificatePair> {
    if let Value::Object(map) = value {
        let mut pair = OpensslTlcpCertificatePair::default();

        for (k, v) in map {
            match crate::key::normalize(k).as_str() {
                "sign_certificate" | "sign_cert" => {
                    let cert = as_openssl_certificates(v)
                        .context(format!("invalid certificates value for key {k}"))?;
                    pair.set_sign_certificates(cert)
                        .context("failed to set sign certificate")?;
                }
                "enc_certificate" | "enc_cert" => {
                    let cert = as_openssl_certificates(v)
                        .context(format!("invalid certificates value for key {k}"))?;
                    pair.set_enc_certificates(cert)
                        .context("failed to set enc certificate")?;
                }
                "sign_private_key" | "sign_key" => {
                    let key = as_openssl_private_key(v)
                        .context(format!("invalid private key value for key {k}"))?;
                    pair.set_sign_private_key(key)
                        .context("failed to set private key")?;
                }
                "enc_private_key" | "enc_key" => {
                    let key = as_openssl_private_key(v)
                        .context(format!("invalid private key value for key {k}"))?;
                    pair.set_enc_private_key(key)
                        .context("failed to set private key")?;
                }
                _ => return Err(anyhow!("invalid key {k}")),
            }
        }

        pair.check()?;
        Ok(pair)
    } else {
        Err(anyhow!(
            "yaml value type for 'openssl tlcp cert pair' should be 'map'"
        ))
    }
}

fn as_openssl_protocol(value: &Value) -> anyhow::Result<OpensslProtocol> {
    if let Value::String(s) = value {
        OpensslProtocol::from_str(s)
    } else {
        Err(anyhow!(
            "json value type for openssl protocol should be 'string'"
        ))
    }
}

fn as_openssl_ciphers(value: &Value) -> anyhow::Result<Vec<String>> {
    let mut ciphers = Vec::new();
    match value {
        Value::String(s) => {
            for cipher in s.split(':') {
                ciphers.push(cipher.to_string());
            }
            Ok(ciphers)
        }
        Value::Array(seq) => {
            for (i, v) in seq.iter().enumerate() {
                if let Value::String(s) = v {
                    ciphers.push(s.to_string());
                } else {
                    return Err(anyhow!("invalid cipher string for #{i}"));
                }
            }
            Ok(ciphers)
        }
        _ => Err(anyhow!(
            "json value type for openssl ciphers should be 'string' or an 'array' of string"
        )),
    }
}

fn set_openssl_tls_client_config_builder(
    mut builder: OpensslClientConfigBuilder,
    value: &Value,
) -> anyhow::Result<OpensslClientConfigBuilder> {
    if let Value::Object(map) = value {
        let mut cert_pair = OpensslCertificatePair::default();

        for (k, v) in map {
            match crate::key::normalize(k).as_str() {
                "protocol" => {
                    let protocol = as_openssl_protocol(v)
                        .context(format!("invalid openssl protocol value for key {k}"))?;
                    builder.set_protocol(protocol);
                }
                "min_tls_version" | "tls_version_min" => {
                    let tls_version = crate::value::as_tls_version(v)
                        .context(format!("invalid tls version value for key {k}"))?;
                    builder.set_min_tls_version(tls_version);
                }
                "max_tls_version" | "tls_version_max" => {
                    let tls_version = crate::value::as_tls_version(v)
                        .context(format!("invalid tls version value for key {k}"))?;
                    builder.set_max_tls_version(tls_version);
                }
                "ciphers" => {
                    let ciphers = as_openssl_ciphers(v)
                        .context(format!("invalid openssl ciphers value for key {k}"))?;
                    builder.set_ciphers(ciphers);
                }
                "disable_sni" => {
                    let disable = crate::value::as_bool(v)
                        .context(format!("invalid bool value for key {k}"))?;
                    if disable {
                        builder.set_disable_sni();
                    }
                }
                "certificate" | "cert" => {
                    let cert = as_openssl_certificates(v)
                        .context(format!("invalid certificates value for key {k}"))?;
                    cert_pair
                        .set_certificates(cert)
                        .context("failed to set certificate")?;
                }
                "private_key" | "key" => {
                    let key = as_openssl_private_key(v)
                        .context(format!("invalid private key value for key {k}"))?;
                    cert_pair
                        .set_private_key(key)
                        .context("failed to set private key")?;
                }
                "cert_pair" => {
                    let pair = as_openssl_certificate_pair(v)
                        .context(format!("invalid cert pair value for key {k}"))?;
                    builder.set_cert_pair(pair);
                }
                "tlcp_cert_pair" => {
                    let pair = as_openssl_tlcp_certificate_pair(v)
                        .context(format!("invalid tlcp certificate pair value for key {k}"))?;
                    builder.set_tlcp_cert_pair(pair);
                }
                "ca_certificate" | "ca_cert" | "server_auth_certificate" | "server_auth_cert" => {
                    let certs = as_openssl_certificates(v)
                        .context(format!("invalid certificates value for key {k}"))?;
                    builder
                        .set_ca_certificates(certs)
                        .context("failed to set ca certificate")?;
                }
                "no_default_ca_certificate" | "no_default_ca_cert" => {
                    let no_default = crate::value::as_bool(v)
                        .context(format!("invalid bool value for key {k}"))?;
                    if no_default {
                        builder.set_no_default_ca_certificates();
                    }
                }
                "handshake_timeout" | "negotiation_timeout" => {
                    let timeout = crate::humanize::as_duration(v)
                        .context(format!("invalid humanize duration value for key {k}"))?;
                    builder.set_handshake_timeout(timeout);
                }
                "no_session_cache" | "disable_session_cache" | "session_cache_disabled" => {
                    let no = crate::value::as_bool(v)
                        .context(format!("invalid bool value for key {k}"))?;
                    if no {
                        builder.set_no_session_cache();
                    }
                }
                "use_builtin_session_cache" => {
                    let yes = crate::value::as_bool(v)
                        .context(format!("invalid bool value for key {k}"))?;
                    if yes {
                        builder.set_use_builtin_session_cache();
                    }
                }
                "session_cache_lru_max_sites" => {
                    let max = crate::value::as_usize(v)
                        .context(format!("invalid usize value for key {k}"))?;
                    builder.set_session_cache_sites_count(max);
                }
                "session_cache_each_capacity" | "session_cache_each_cap" => {
                    let cap = crate::value::as_usize(v)
                        .context(format!("invalid usize value for key {k}"))?;
                    builder.set_session_cache_each_capacity(cap);
                }
                "supported_groups" => {
                    let groups = crate::value::as_string(v)?;
                    builder.set_supported_groups(groups);
                }
                "use_ocsp_stapling" => {
                    let enable = crate::value::as_bool(v)?;
                    builder.set_use_ocsp_stapling(enable);
                }
                "enable_sct" => {
                    let enable = crate::value::as_bool(v)?;
                    builder.set_enable_sct(enable);
                }
                "enable_grease" => {
                    let enable = crate::value::as_bool(v)?;
                    builder.set_enable_grease(enable);
                }
                "permute_extensions" => {
                    let enable = crate::value::as_bool(v)?;
                    builder.set_permute_extensions(enable);
                }
                "insecure" => {
                    let enable = crate::value::as_bool(v)?;
                    builder.set_insecure(enable);
                }
                _ => return Err(anyhow!("invalid key {k}")),
            }
        }

        if cert_pair.is_set() && builder.set_cert_pair(cert_pair).is_some() {
            return Err(anyhow!("found duplicate client certificate config"));
        }

        builder.check()?;
        Ok(builder)
    } else {
        Err(anyhow!(
            "json value type for 'openssl tls client config builder' should be 'map'"
        ))
    }
}

pub fn as_to_one_openssl_tls_client_config_builder(
    value: &Value,
) -> anyhow::Result<OpensslClientConfigBuilder> {
    let builder = OpensslClientConfigBuilder::with_cache_for_one_site();
    set_openssl_tls_client_config_builder(builder, value)
}

pub fn as_to_many_openssl_tls_client_config_builder(
    value: &Value,
) -> anyhow::Result<OpensslClientConfigBuilder> {
    let builder = OpensslClientConfigBuilder::with_cache_for_many_sites();
    set_openssl_tls_client_config_builder(builder, value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use g3_types::net::TlsVersion;
    use serde_json::json;
    use std::time::Duration;

    const TEST_CERT_PEM1: &str = include_str!("test_data/test_cert1.pem");
    const TEST_CERT_PEM2: &str = include_str!("test_data/test_cert2.pem");
    const TEST_KEY_PEM1: &str = include_str!("test_data/test_key1.pem");
    const TEST_KEY_PEM2: &str = include_str!("test_data/test_key2.pem");

    #[test]
    fn as_openssl_certificates_ok() {
        // Single certificate string
        let value = json!(TEST_CERT_PEM1);
        let certs = as_openssl_certificates(&value).unwrap();
        assert!(!certs.is_empty());

        // Array of certificates
        let value = json!([TEST_CERT_PEM1, TEST_CERT_PEM2]);
        let certs = as_openssl_certificates(&value).unwrap();
        assert_eq!(certs.len(), 2);
    }

    #[test]
    fn as_openssl_certificates_err() {
        // Invalid type
        let value = json!({});
        assert!(as_openssl_certificates(&value).is_err());

        // Empty certificate string
        let value = json!("");
        assert!(as_openssl_certificates(&value).is_err());

        // Invalid PEM format
        let value = json!("invalid");
        assert!(as_openssl_certificates(&value).is_err());
    }

    #[test]
    fn as_openssl_private_key_ok() {
        let value = json!(TEST_KEY_PEM1);
        let key = as_openssl_private_key(&value).unwrap();
        assert!(key.private_key_to_pem_pkcs8().is_ok());
    }

    #[test]
    fn as_openssl_private_key_err() {
        // Invalid type
        let value = json!(123);
        assert!(as_openssl_private_key(&value).is_err());

        // Invalid key format
        let value = json!("invalid_key");
        assert!(as_openssl_private_key(&value).is_err());
    }

    #[test]
    fn as_openssl_certificate_pair_ok() {
        let value = json!({
            "certificate": TEST_CERT_PEM1,
            "private_key": TEST_KEY_PEM1
        });
        let pair = as_openssl_certificate_pair(&value).unwrap();
        assert!(pair.is_set());
    }

    #[test]
    fn as_openssl_certificate_pair_err() {
        // Missing required fields
        let value = json!({});
        assert!(as_openssl_certificate_pair(&value).is_err());

        // Invalid certificate
        let value = json!({
            "certificate": "invalid",
            "private_key": TEST_KEY_PEM1
        });
        assert!(as_openssl_certificate_pair(&value).is_err());

        // Invalid private key
        let value = json!({
            "certificate": TEST_CERT_PEM1,
            "private_key": "invalid"
        });
        assert!(as_openssl_certificate_pair(&value).is_err());

        // Extra fields
        let value = json!({
            "certificate": TEST_CERT_PEM1,
            "private_key": TEST_KEY_PEM1,
            "extra": "field"
        });
        assert!(as_openssl_certificate_pair(&value).is_err());

        // Invalid value type
        let value = json!(123);
        assert!(as_openssl_certificate_pair(&value).is_err());
    }

    #[test]
    fn as_openssl_tlcp_certificate_pair_ok() {
        let value = json!({
            "sign_certificate": TEST_CERT_PEM1,
            "enc_certificate": TEST_CERT_PEM2,
            "sign_private_key": TEST_KEY_PEM1,
            "enc_private_key": TEST_KEY_PEM2
        });
        let pair = as_openssl_tlcp_certificate_pair(&value).unwrap();
        assert!(pair.check().is_ok());
    }

    #[test]
    fn as_openssl_tlcp_certificate_pair_err() {
        // Missing required fields
        let value = json!({
            "sign_certificate": TEST_CERT_PEM1,
            "enc_certificate": TEST_CERT_PEM2,
            "sign_private_key": TEST_KEY_PEM1
        });
        assert!(as_openssl_tlcp_certificate_pair(&value).is_err());

        // Invalid key
        let value = json!({
            "invalid_key": "value"
        });
        assert!(as_openssl_tlcp_certificate_pair(&value).is_err());

        // Invalid value type
        let value = json!(123);
        assert!(as_openssl_tlcp_certificate_pair(&value).is_err());
    }

    #[test]
    fn as_to_one_openssl_tls_client_config_builder_ok() {
        let value = json!({
            "protocol": "tls12",
            "min_tls_version": "tls1.2",
            "max_tls_version": "tls1.3",
            "ciphers": ["TLS_AES_128_GCM_SHA256"],
            "disable_sni": true,
            "cert_pair": {
                "certificate": TEST_CERT_PEM1,
                "private_key": TEST_KEY_PEM1
            },
            "ca_certificate": TEST_CERT_PEM2,
            "no_default_ca_certificate": true,
            "handshake_timeout": "10s",
            "no_session_cache": true,
            "session_cache_lru_max_sites": 100,
            "session_cache_each_capacity": 10,
            "supported_groups": "P-256",
            "use_ocsp_stapling": true,
            "enable_sct": true,
            "enable_grease": true,
            "permute_extensions": true,
            "insecure": false
        });
        let builder = as_to_one_openssl_tls_client_config_builder(&value).unwrap();
        let mut expected = OpensslClientConfigBuilder::default();
        expected.set_protocol(OpensslProtocol::Tls12);
        expected.set_min_tls_version(TlsVersion::TLS1_2);
        expected.set_max_tls_version(TlsVersion::TLS1_3);
        expected.set_ciphers(vec!["TLS_AES_128_GCM_SHA256".to_string()]);
        expected.set_disable_sni();
        let value = json!({
            "certificate": TEST_CERT_PEM1,
            "private_key": TEST_KEY_PEM1
        });
        let cert_pair = as_openssl_certificate_pair(&value).unwrap();
        expected.set_cert_pair(cert_pair);
        let ca_certs = as_openssl_certificates(&json!(TEST_CERT_PEM2)).unwrap();
        expected.set_ca_certificates(ca_certs).unwrap();
        expected.set_no_default_ca_certificates();
        expected.set_handshake_timeout(Duration::from_secs(10));
        expected.set_no_session_cache();
        expected.set_session_cache_sites_count(100);
        expected.set_session_cache_each_capacity(10);
        expected.set_supported_groups("P-256".to_string());
        expected.set_use_ocsp_stapling(true);
        expected.set_enable_sct(true);
        expected.set_enable_grease(true);
        expected.set_permute_extensions(true);
        expected.set_insecure(false);
        assert_eq!(builder, expected);
    }

    #[test]
    fn as_to_one_openssl_tls_client_config_builder_err() {
        // Invalid value type for protocol
        let value = json!({"protocol": 123});
        assert!(as_to_one_openssl_tls_client_config_builder(&value).is_err());

        // Duplicate certificate config
        let value = json!({
            "certificate": TEST_CERT_PEM1,
            "private_key": TEST_KEY_PEM1,
            "cert_pair": {
                "certificate": TEST_CERT_PEM2,
                "private_key": TEST_KEY_PEM2
            }
        });
        assert!(as_to_one_openssl_tls_client_config_builder(&value).is_err());
    }

    #[test]
    fn as_to_many_openssl_tls_client_config_builder_ok() {
        let value = json!({
            "protocol": "tls13",
            "tls_version_min": "tls1.2",
            "tls_version_max": "tls1.3",
            "ciphers": "TLS_AES_256_GCM_SHA384:TLS_CHACHA20_POLY1305_SHA256",
            "disable_sni": false,
            "cert": TEST_CERT_PEM1,
            "key": TEST_KEY_PEM1,
            "tlcp_cert_pair": {
                "sign_certificate": TEST_CERT_PEM1,
                "enc_certificate": TEST_CERT_PEM2,
                "sign_private_key": TEST_KEY_PEM1,
                "enc_private_key": TEST_KEY_PEM2
            },
            "ca_cert": TEST_CERT_PEM2,
            "no_default_ca_cert": true,
            "negotiation_timeout": "5s",
            "disable_session_cache": false,
            "use_builtin_session_cache": true,
            "session_cache_lru_max_sites": 50,
            "session_cache_each_cap": 20,
            "supported_groups": "X25519:P-384",
            "use_ocsp_stapling": false,
            "enable_sct": false,
            "enable_grease": false,
            "permute_extensions": false,
            "insecure": true
        });
        let builder = as_to_many_openssl_tls_client_config_builder(&value).unwrap();
        let mut expected = OpensslClientConfigBuilder::default();
        expected.set_protocol(OpensslProtocol::Tls13);
        expected.set_min_tls_version(TlsVersion::TLS1_2);
        expected.set_max_tls_version(TlsVersion::TLS1_3);
        expected.set_ciphers(vec![
            "TLS_AES_256_GCM_SHA384".to_string(),
            "TLS_CHACHA20_POLY1305_SHA256".to_string(),
        ]);
        let cert_pair = as_openssl_certificate_pair(&json!({
            "certificate": TEST_CERT_PEM1,
            "private_key": TEST_KEY_PEM1
        }))
        .unwrap();
        expected.set_cert_pair(cert_pair);
        let tlcp_cert_pair = as_openssl_tlcp_certificate_pair(&json!({
            "sign_certificate": TEST_CERT_PEM1,
            "enc_certificate": TEST_CERT_PEM2,
            "sign_private_key": TEST_KEY_PEM1,
            "enc_private_key": TEST_KEY_PEM2
        }))
        .unwrap();
        expected.set_tlcp_cert_pair(tlcp_cert_pair);
        let ca_certs = as_openssl_certificates(&json!(TEST_CERT_PEM2)).unwrap();
        expected.set_ca_certificates(ca_certs).unwrap();
        expected.set_no_default_ca_certificates();
        expected.set_handshake_timeout(Duration::from_secs(5));
        expected.set_use_builtin_session_cache();
        expected.set_session_cache_sites_count(50);
        expected.set_session_cache_each_capacity(20);
        expected.set_supported_groups("X25519:P-384".to_string());
        expected.set_use_ocsp_stapling(false);
        expected.set_enable_sct(false);
        expected.set_enable_grease(false);
        expected.set_permute_extensions(false);
        expected.set_insecure(true);
        assert_eq!(builder, expected);
    }

    #[test]
    fn as_to_many_openssl_tls_client_config_builder_err() {
        // Invalid ciphers format
        let value = json!({"ciphers": 123});
        assert!(as_to_many_openssl_tls_client_config_builder(&value).is_err());

        // Invalid cipher string
        let value = json!({"ciphers": "invalid_cipher"});
        assert!(as_to_many_openssl_tls_client_config_builder(&value).is_err());

        // Missing required fields for cache
        let value = json!({
            "session_cache_lru_max_sites": 100,
            "session_cache_each_capacity": 10
        });
        assert!(as_to_many_openssl_tls_client_config_builder(&value).is_ok());

        // Invalid key
        let value = json!({"invalid_key": "value"});
        assert!(as_to_many_openssl_tls_client_config_builder(&value).is_err());

        // Invalid value type
        let value = json!(123);
        assert!(as_to_many_openssl_tls_client_config_builder(&value).is_err());
    }
}
