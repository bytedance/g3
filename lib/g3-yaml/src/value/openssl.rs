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

use std::io::Read;
use std::path::Path;
use std::str::FromStr;

use anyhow::{anyhow, Context};
use openssl::pkey::{PKey, Private};
use openssl::x509::X509;
use yaml_rust::Yaml;

use g3_types::net::{
    OpensslCertificatePair, OpensslClientConfigBuilder, OpensslInterceptionClientConfigBuilder,
    OpensslInterceptionServerConfigBuilder, OpensslProtocol, OpensslServerConfigBuilder,
};

#[cfg(feature = "tongsuo")]
use g3_types::net::OpensslTlcpCertificatePair;

fn as_certificates_from_single_element(
    value: &Yaml,
    lookup_dir: Option<&Path>,
) -> anyhow::Result<Vec<X509>> {
    const MAX_FILE_SIZE: usize = 4_000_000; // 4MB

    if let Yaml::String(s) = value {
        if s.trim_start().starts_with("--") {
            let certs = X509::stack_from_pem(s.as_bytes())
                .map_err(|e| anyhow!("invalid certificate string: {e}"))?;
            return if certs.is_empty() {
                Err(anyhow!("no valid certificate found"))
            } else {
                Ok(certs)
            };
        }
    }

    let (file, path) = crate::value::as_file(value, lookup_dir).context("invalid file")?;
    let mut contents = String::with_capacity(MAX_FILE_SIZE);
    file.take(MAX_FILE_SIZE as u64)
        .read_to_string(&mut contents)
        .map_err(|e| anyhow!("failed to read contents of file {}: {e}", path.display()))?;
    let certs = X509::stack_from_pem(contents.as_bytes())
        .map_err(|e| anyhow!("invalid certificate file({}): {e}", path.display()))?;
    if certs.is_empty() {
        Err(anyhow!(
            "no valid certificate found in file {}",
            path.display()
        ))
    } else {
        Ok(certs)
    }
}

pub fn as_openssl_certificates(
    value: &Yaml,
    lookup_dir: Option<&Path>,
) -> anyhow::Result<Vec<X509>> {
    if let Yaml::Array(seq) = value {
        let mut certs = Vec::new();
        for (i, v) in seq.iter().enumerate() {
            let this_certs = as_certificates_from_single_element(v, lookup_dir)
                .context(format!("invalid certificates value for element #{i}"))?;
            certs.extend(this_certs);
        }
        Ok(certs)
    } else {
        as_certificates_from_single_element(value, lookup_dir)
    }
}

pub fn as_openssl_private_key(
    value: &Yaml,
    lookup_dir: Option<&Path>,
) -> anyhow::Result<PKey<Private>> {
    const MAX_FILE_SIZE: usize = 256_000; // 256KB

    if let Yaml::String(s) = value {
        if s.trim_start().starts_with("--") {
            return PKey::private_key_from_pem(s.as_bytes())
                .map_err(|e| anyhow!("invalid private key string: {e}"));
        }
    }

    let (file, path) = crate::value::as_file(value, lookup_dir).context("invalid file")?;
    let mut contents = String::with_capacity(MAX_FILE_SIZE);
    file.take(MAX_FILE_SIZE as u64)
        .read_to_string(&mut contents)
        .map_err(|e| anyhow!("failed to read contents of file {}: {e}", path.display()))?;
    PKey::private_key_from_pem(contents.as_bytes())
        .map_err(|e| anyhow!("invalid private key file({}): {e}", path.display()))
}

pub fn as_openssl_certificate_pair(
    value: &Yaml,
    lookup_dir: Option<&Path>,
) -> anyhow::Result<OpensslCertificatePair> {
    if let Yaml::Hash(map) = value {
        let mut pair = OpensslCertificatePair::default();

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "certificate" | "cert" => {
                let cert = as_openssl_certificates(v, lookup_dir)
                    .context(format!("invalid certificates value for key {k}"))?;
                pair.set_certificates(cert)
                    .context("failed to set certificate")?;
                Ok(())
            }
            "private_key" | "key" => {
                let key = as_openssl_private_key(v, lookup_dir)
                    .context(format!("invalid private key value for key {k}"))?;
                pair.set_private_key(key)
                    .context("failed to set private key")?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        pair.check()?;
        Ok(pair)
    } else {
        Err(anyhow!(
            "yaml value type for openssl certificate pair should be 'map'"
        ))
    }
}

#[cfg(feature = "tongsuo")]
pub fn as_openssl_tlcp_certificate_pair(
    value: &Yaml,
    lookup_dir: Option<&Path>,
) -> anyhow::Result<OpensslTlcpCertificatePair> {
    if let Yaml::Hash(map) = value {
        let mut pair = OpensslTlcpCertificatePair::default();

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "sign_certificate" | "sign_cert" => {
                let cert = as_openssl_certificates(v, lookup_dir)
                    .context(format!("invalid certificates value for key {k}"))?;
                pair.set_sign_certificates(cert)
                    .context("failed to set sign certificate")?;
                Ok(())
            }
            "enc_certificate" | "enc_cert" => {
                let cert = as_openssl_certificates(v, lookup_dir)
                    .context(format!("invalid certificates value for key {k}"))?;
                pair.set_enc_certificates(cert)
                    .context("failed to set enc certificate")?;
                Ok(())
            }
            "sign_private_key" | "sign_key" => {
                let key = as_openssl_private_key(v, lookup_dir)
                    .context(format!("invalid private key value for key {k}"))?;
                pair.set_sign_private_key(key)
                    .context("failed to set private key")?;
                Ok(())
            }
            "enc_private_key" | "enc_key" => {
                let key = as_openssl_private_key(v, lookup_dir)
                    .context(format!("invalid private key value for key {k}"))?;
                pair.set_enc_private_key(key)
                    .context("failed to set private key")?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        pair.check()?;
        Ok(pair)
    } else {
        Err(anyhow!(
            "yaml value type for 'openssl tlcp cert pair' should be 'map'"
        ))
    }
}

fn as_openssl_protocol(value: &Yaml) -> anyhow::Result<OpensslProtocol> {
    if let Yaml::String(s) = value {
        OpensslProtocol::from_str(s)
    } else {
        Err(anyhow!(
            "yaml value type for openssl protocol should be 'string'"
        ))
    }
}

fn as_openssl_ciphers(value: &Yaml) -> anyhow::Result<Vec<String>> {
    let mut ciphers = Vec::new();
    match value {
        Yaml::String(s) => {
            for cipher in s.split(':') {
                ciphers.push(cipher.to_string());
            }
            Ok(ciphers)
        }
        Yaml::Array(seq) => {
            for (i, v) in seq.iter().enumerate() {
                if let Yaml::String(s) = v {
                    ciphers.push(s.to_string());
                } else {
                    return Err(anyhow!("invalid cipher string for #{i}"));
                }
            }
            Ok(ciphers)
        }
        _ => Err(anyhow!(
            "yaml value type for openssl ciphers should be 'string' or an 'array' of string"
        )),
    }
}

fn set_openssl_tls_client_config_builder(
    mut builder: OpensslClientConfigBuilder,
    value: &Yaml,
    lookup_dir: Option<&Path>,
) -> anyhow::Result<OpensslClientConfigBuilder> {
    if let Yaml::Hash(map) = value {
        let mut cert_pair = OpensslCertificatePair::default();

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "protocol" => {
                let protocol = as_openssl_protocol(v)
                    .context(format!("invalid openssl protocol value for key {k}"))?;
                builder.set_protocol(protocol);
                Ok(())
            }
            "min_tls_version" | "tls_version_min" => {
                let tls_version = crate::value::as_tls_version(v)
                    .context(format!("invalid tls version value for key {k}"))?;
                builder.set_min_tls_version(tls_version);
                Ok(())
            }
            "max_tls_version" | "tls_version_max" => {
                let tls_version = crate::value::as_tls_version(v)
                    .context(format!("invalid tls version value for key {k}"))?;
                builder.set_max_tls_version(tls_version);
                Ok(())
            }
            "ciphers" => {
                let ciphers = as_openssl_ciphers(v)
                    .context(format!("invalid openssl ciphers value for key {k}"))?;
                builder.set_ciphers(ciphers);
                Ok(())
            }
            "disable_sni" => {
                let disable =
                    crate::value::as_bool(v).context(format!("invalid bool value for key {k}"))?;
                if disable {
                    builder.set_disable_sni();
                }
                Ok(())
            }
            "certificate" | "cert" => {
                let cert = as_openssl_certificates(v, lookup_dir)
                    .context(format!("invalid certificates value for key {k}"))?;
                cert_pair
                    .set_certificates(cert)
                    .context("failed to set certificate")?;
                Ok(())
            }
            "private_key" | "key" => {
                let key = as_openssl_private_key(v, lookup_dir)
                    .context(format!("invalid private key value for key {k}"))?;
                cert_pair
                    .set_private_key(key)
                    .context("failed to set private key")?;
                Ok(())
            }
            "cert_pair" => {
                let pair = as_openssl_certificate_pair(v, lookup_dir)
                    .context(format!("invalid cert pair value for key {k}"))?;
                builder.set_cert_pair(pair);
                Ok(())
            }
            #[cfg(feature = "tongsuo")]
            "tlcp_cert_pair" => {
                let pair = as_openssl_tlcp_certificate_pair(v, lookup_dir)
                    .context(format!("invalid tlcp certificate pair value for key {k}"))?;
                builder.set_tlcp_cert_pair(pair);
                Ok(())
            }
            "ca_certificate" | "ca_cert" | "server_auth_certificate" | "server_auth_cert" => {
                let certs = as_openssl_certificates(v, lookup_dir)
                    .context(format!("invalid certificates value for key {k}"))?;
                builder
                    .set_ca_certificates(certs)
                    .context("failed to set ca certificate")?;
                Ok(())
            }
            "no_default_ca_certificate" | "no_default_ca_cert" => {
                let no_default =
                    crate::value::as_bool(v).context(format!("invalid bool value for key {k}"))?;
                if no_default {
                    builder.set_no_default_ca_certificates();
                }
                Ok(())
            }
            "handshake_timeout" | "negotiation_timeout" => {
                let timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                builder.set_handshake_timeout(timeout);
                Ok(())
            }
            "no_session_cache" | "disable_session_cache" | "session_cache_disabled" => {
                let no =
                    crate::value::as_bool(v).context(format!("invalid bool value for key {k}"))?;
                if no {
                    builder.set_no_session_cache();
                }
                Ok(())
            }
            "use_builtin_session_cache" => {
                let yes =
                    crate::value::as_bool(v).context(format!("invalid bool value for key {k}"))?;
                if yes {
                    builder.set_use_builtin_session_cache();
                }
                Ok(())
            }
            "session_cache_lru_max_sites" => {
                let max = crate::value::as_usize(v)
                    .context(format!("invalid usize value for key {k}"))?;
                builder.set_session_cache_sites_count(max);
                Ok(())
            }
            "session_cache_each_capacity" | "session_cache_each_cap" => {
                let cap = crate::value::as_usize(v)
                    .context(format!("invalid usize value for key {k}"))?;
                builder.set_session_cache_each_capacity(cap);
                Ok(())
            }
            "supported_groups" => {
                let groups = crate::value::as_string(v)?;
                builder.set_supported_groups(groups);
                Ok(())
            }
            "use_ocsp_stapling" => {
                let enable = crate::value::as_bool(v)?;
                builder.set_use_ocsp_stapling(enable);
                Ok(())
            }
            "enable_sct" => {
                let enable = crate::value::as_bool(v)?;
                builder.set_enable_sct(enable);
                Ok(())
            }
            "enable_grease" => {
                let enable = crate::value::as_bool(v)?;
                builder.set_enable_grease(enable);
                Ok(())
            }
            "permute_extensions" => {
                let enable = crate::value::as_bool(v)?;
                builder.set_permute_extensions(enable);
                Ok(())
            }
            "insecure" => {
                let enable = crate::value::as_bool(v)?;
                builder.set_insecure(enable);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        if cert_pair.is_set() && builder.set_cert_pair(cert_pair).is_some() {
            return Err(anyhow!("found duplicate client certificate config"));
        }

        builder.check()?;
        Ok(builder)
    } else {
        Err(anyhow!(
            "yaml value type for 'openssl tls client config builder' should be 'map'"
        ))
    }
}

pub fn as_to_one_openssl_tls_client_config_builder(
    value: &Yaml,
    lookup_dir: Option<&Path>,
) -> anyhow::Result<OpensslClientConfigBuilder> {
    let builder = OpensslClientConfigBuilder::with_cache_for_one_site();
    set_openssl_tls_client_config_builder(builder, value, lookup_dir)
}

pub fn as_to_many_openssl_tls_client_config_builder(
    value: &Yaml,
    lookup_dir: Option<&Path>,
) -> anyhow::Result<OpensslClientConfigBuilder> {
    let builder = OpensslClientConfigBuilder::with_cache_for_many_sites();
    set_openssl_tls_client_config_builder(builder, value, lookup_dir)
}

pub fn as_tls_interception_client_config_builder(
    value: &Yaml,
    lookup_dir: Option<&Path>,
) -> anyhow::Result<OpensslInterceptionClientConfigBuilder> {
    if let Yaml::Hash(map) = value {
        let mut builder = OpensslInterceptionClientConfigBuilder::default();

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "min_tls_version" | "tls_version_min" => {
                let tls_version = crate::value::as_tls_version(v)
                    .context(format!("invalid tls version value for key {k}"))?;
                builder.set_min_tls_version(tls_version);
                Ok(())
            }
            "max_tls_version" | "tls_version_max" => {
                let tls_version = crate::value::as_tls_version(v)
                    .context(format!("invalid tls version value for key {k}"))?;
                builder.set_max_tls_version(tls_version);
                Ok(())
            }
            "ca_certificate" | "ca_cert" | "server_auth_certificate" | "server_auth_cert" => {
                let certs = as_openssl_certificates(v, lookup_dir)
                    .context(format!("invalid certificates value for key {k}"))?;
                builder
                    .set_ca_certificates(certs)
                    .context("failed to set ca certificate")?;
                Ok(())
            }
            "no_default_ca_certificate" | "no_default_ca_cert" => {
                let no_default =
                    crate::value::as_bool(v).context(format!("invalid bool value for key {k}"))?;
                if no_default {
                    builder.set_no_default_ca_certificates();
                }
                Ok(())
            }
            "handshake_timeout" | "negotiation_timeout" => {
                let timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                builder.set_handshake_timeout(timeout);
                Ok(())
            }
            "no_session_cache" | "disable_session_cache" | "session_cache_disabled" => {
                let no =
                    crate::value::as_bool(v).context(format!("invalid bool value for key {k}"))?;
                if no {
                    builder.set_no_session_cache();
                }
                Ok(())
            }
            "session_cache_lru_max_sites" => {
                let max = crate::value::as_usize(v)
                    .context(format!("invalid usize value for key {k}"))?;
                builder.set_session_cache_sites_count(max);
                Ok(())
            }
            "session_cache_each_capacity" | "session_cache_each_cap" => {
                let cap = crate::value::as_usize(v)
                    .context(format!("invalid usize value for key {k}"))?;
                builder.set_session_cache_each_capacity(cap);
                Ok(())
            }
            "supported_groups" => {
                let groups = crate::value::as_string(v)?;
                builder.set_supported_groups(groups);
                Ok(())
            }
            "use_ocsp_stapling" => {
                let enable = crate::value::as_bool(v)?;
                builder.set_use_ocsp_stapling(enable);
                Ok(())
            }
            "enable_sct" => {
                let enable = crate::value::as_bool(v)?;
                builder.set_enable_sct(enable);
                Ok(())
            }
            "enable_grease" => {
                let enable = crate::value::as_bool(v)?;
                builder.set_enable_grease(enable);
                Ok(())
            }
            "permute_extensions" => {
                let enable = crate::value::as_bool(v)?;
                builder.set_permute_extensions(enable);
                Ok(())
            }
            "insecure" => {
                let enable = crate::value::as_bool(v)?;
                builder.set_insecure(enable);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        builder.check()?;
        Ok(builder)
    } else {
        Err(anyhow!(
            "yaml value type for 'openssl tls interception client config builder' should be 'map'"
        ))
    }
}

pub fn as_openssl_tls_server_config_builder(
    value: &Yaml,
    lookup_dir: Option<&Path>,
) -> anyhow::Result<OpensslServerConfigBuilder> {
    if let Yaml::Hash(map) = value {
        let mut builder = OpensslServerConfigBuilder::empty();

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "cert_pairs" => {
                if let Yaml::Array(seq) = v {
                    for (i, v) in seq.iter().enumerate() {
                        let pair = as_openssl_certificate_pair(v, lookup_dir)
                            .context(format!("invalid openssl cert pair value for {k}#{i}"))?;
                        builder
                            .push_cert_pair(pair)
                            .context(format!("invalid openssl cert pair value for {k}#{i}"))?;
                    }
                } else {
                    let pair = as_openssl_certificate_pair(v, lookup_dir)
                        .context(format!("invalid openssl cert pair value for key {k}"))?;
                    builder
                        .push_cert_pair(pair)
                        .context(format!("invalid openssl cert pair value for key {k}"))?;
                }
                Ok(())
            }
            #[cfg(feature = "tongsuo")]
            "tlcp_cert_pairs" => {
                if let Yaml::Array(seq) = v {
                    for (i, v) in seq.iter().enumerate() {
                        let pair = as_openssl_tlcp_certificate_pair(v, lookup_dir)
                            .context(format!("invalid openssl tlcp cert pair value for {k}#{i}"))?;
                        builder
                            .push_tlcp_cert_pair(pair)
                            .context(format!("invalid openssl tlcp cert pair value for {k}#{i}"))?;
                    }
                } else {
                    let pair = as_openssl_tlcp_certificate_pair(v, lookup_dir)
                        .context(format!("invalid openssl tlcp cert pair value for key {k}"))?;
                    builder
                        .push_tlcp_cert_pair(pair)
                        .context(format!("invalid openssl tlcp cert pair value for key {k}"))?;
                }
                Ok(())
            }
            "enable_client_auth" => {
                let enable =
                    crate::value::as_bool(v).context(format!("invalid value for key {k}"))?;
                if enable {
                    builder.enable_client_auth();
                }
                Ok(())
            }
            "session_id_context" => {
                let context = crate::value::as_string(v)?;
                builder.set_session_id_context(context);
                Ok(())
            }
            "no_session_ticket" | "disable_session_ticket" => {
                let disable = crate::value::as_bool(v)?;
                builder.set_disable_session_ticket(disable);
                Ok(())
            }
            "no_session_cache" | "disable_session_cache" => {
                let disable = crate::value::as_bool(v)?;
                builder.set_disable_session_cache(disable);
                Ok(())
            }
            "ca_certificate" | "ca_cert" | "client_auth_certificate" | "client_auth_cert" => {
                let certs = as_openssl_certificates(v, lookup_dir)
                    .context(format!("invalid value for key {k}"))?;
                builder.set_client_auth_certificates(certs)
            }
            "handshake_timeout" | "negotiation_timeout" | "accept_timeout" => {
                let timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                builder.set_accept_timeout(timeout);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        builder.check()?;
        Ok(builder)
    } else {
        Err(anyhow!(
            "yaml value type for 'openssl server config builder' should be 'map'"
        ))
    }
}

pub fn as_tls_interception_server_config_builder(
    value: &Yaml,
) -> anyhow::Result<OpensslInterceptionServerConfigBuilder> {
    if let Yaml::Hash(map) = value {
        let mut builder = OpensslInterceptionServerConfigBuilder::default();

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "handshake_timeout" | "negotiation_timeout" | "accept_timeout" => {
                let timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                builder.set_accept_timeout(timeout);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        builder.check()?;
        Ok(builder)
    } else {
        Err(anyhow!(
            "yaml value type for 'openssl tls interception server config builder' should be 'map'"
        ))
    }
}
