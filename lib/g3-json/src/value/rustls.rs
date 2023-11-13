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

use std::io::{BufRead, BufReader};

use anyhow::{anyhow, Context};
use rustls_pemfile::Item;
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
        for (i, r) in rustls_pemfile::certs(&mut BufReader::new(s.as_bytes())).enumerate() {
            let cert = r.map_err(|e| anyhow!("invalid certificate #{i}: {e}"))?;
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

fn read_first_private_key<R>(reader: &mut R) -> anyhow::Result<PrivateKeyDer<'static>>
where
    R: BufRead,
{
    loop {
        match rustls_pemfile::read_one(reader)
            .map_err(|e| anyhow!("read private key failed: {e:?}"))?
        {
            Some(Item::Pkcs1Key(d)) => return Ok(PrivateKeyDer::Pkcs1(d)),
            Some(Item::Pkcs8Key(d)) => return Ok(PrivateKeyDer::Pkcs8(d)),
            Some(Item::Sec1Key(d)) => return Ok(PrivateKeyDer::Sec1(d)),
            Some(_) => continue,
            None => return Err(anyhow!("no valid private key found")),
        }
    }
}

pub fn as_rustls_private_key(value: &Value) -> anyhow::Result<PrivateKeyDer<'static>> {
    if let Value::String(s) = value {
        read_first_private_key(&mut BufReader::new(s.as_bytes()))
            .context("invalid private key string")
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

        if let Ok(cert_pair) = cert_pair_builder.build() {
            if builder.set_cert_pair(cert_pair).is_some() {
                return Err(anyhow!("found duplicate client certificate config"));
            }
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
                        let pair = as_rustls_certificate_pair(value)
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
