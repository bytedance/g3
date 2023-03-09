/*
 * Copyright 2023 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::str::FromStr;

use anyhow::{anyhow, Context};
use openssl::pkey::{PKey, Private};
use openssl::x509::X509;
use serde_json::Value;

use g3_types::net::{OpensslCertificatePair, OpensslProtocol, OpensslTlsClientConfigBuilder};

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
    mut builder: OpensslTlsClientConfigBuilder,
    value: &Value,
) -> anyhow::Result<OpensslTlsClientConfigBuilder> {
    if let Value::Object(map) = value {
        let mut cert_pair = OpensslCertificatePair::default();

        for (k, v) in map {
            match crate::key::normalize(k).as_str() {
                "protocol" => {
                    let protocol = as_openssl_protocol(v)
                        .context(format!("invalid openssl protocol value for key {k}"))?;
                    builder.set_protocol(protocol);
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
) -> anyhow::Result<OpensslTlsClientConfigBuilder> {
    let builder = OpensslTlsClientConfigBuilder::with_cache_for_one_site();
    set_openssl_tls_client_config_builder(builder, value)
}

pub fn as_to_many_openssl_tls_client_config_builder(
    value: &Value,
) -> anyhow::Result<OpensslTlsClientConfigBuilder> {
    let builder = OpensslTlsClientConfigBuilder::with_cache_for_many_sites();
    set_openssl_tls_client_config_builder(builder, value)
}
