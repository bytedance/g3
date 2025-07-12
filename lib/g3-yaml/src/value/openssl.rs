/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io::Read;
use std::path::Path;
use std::str::FromStr;

use anyhow::{Context, anyhow};
use openssl::pkey::{PKey, Private};
use openssl::x509::X509;
use yaml_rust::Yaml;

use g3_types::net::{
    OpensslCertificatePair, OpensslClientConfigBuilder, OpensslInterceptionClientConfigBuilder,
    OpensslInterceptionServerConfigBuilder, OpensslProtocol, OpensslServerConfigBuilder,
    OpensslTlcpCertificatePair,
};

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

#[cfg(test)]
mod tests {
    use super::*;
    use g3_types::net::TlsVersion;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use yaml_rust::YamlLoader;

    const TEST_CERT_PEM1: &str = include_str!("./test_data/test_cert1.pem");
    const TEST_CERT_PEM2: &str = include_str!("./test_data/test_cert2.pem");
    const TEST_KEY_PEM1: &str = include_str!("./test_data/test_key1.pem");
    const TEST_KEY_PEM2: &str = include_str!("./test_data/test_key2.pem");

    static TEST_DIR_ID_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new(prefix: &str) -> Self {
            let id = TEST_DIR_ID_COUNTER.fetch_add(1, Ordering::SeqCst);
            let path =
                std::env::temp_dir().join(format!("{}_{}_{}", prefix, std::process::id(), id));
            fs::create_dir_all(&path).expect("Failed to create test directory");
            TempDir { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn as_openssl_certificates_ok() {
        let temp_dir = TempDir::new("openssl_ok");
        let test_dir_path = temp_dir.path();
        let cert_path = test_dir_path.join("test_cert.pem");
        fs::write(&cert_path, TEST_CERT_PEM1).unwrap();

        // string value (PEM)
        let yaml = Yaml::String(TEST_CERT_PEM1.to_string());
        let certs = as_openssl_certificates(&yaml, None).unwrap();
        assert!(!certs.is_empty());

        // file path
        let yaml = YamlLoader::load_from_str(&format!("|-\n  {}", cert_path.display())).unwrap();
        let certs = as_openssl_certificates(&yaml[0], Some(test_dir_path)).unwrap();
        assert!(!certs.is_empty());

        // array of strings
        let yaml = Yaml::Array(vec![
            Yaml::String(TEST_CERT_PEM1.to_string()),
            Yaml::String(TEST_CERT_PEM1.to_string()),
        ]);
        let certs = as_openssl_certificates(&yaml, None).unwrap();
        assert_eq!(certs.len(), 2);
    }

    #[test]
    fn as_openssl_certificates_err() {
        // invalid string value
        let yaml = yaml_str!("invalid_cert");
        assert!(as_openssl_certificates(&yaml, None).is_err());

        // non-existent file
        let yaml = yaml_str!("non_existent_file.pem");
        assert!(as_openssl_certificates(&yaml, Some(Path::new("/non_existent_dir"))).is_err());

        // empty array
        let yaml = yaml_doc!(r#"- []"#);
        assert!(as_openssl_certificates(&yaml, None).is_err());

        // invalid array element
        let yaml = yaml_doc!(r#"- [123]"#);
        assert!(as_openssl_certificates(&yaml, None).is_err());

        // certificate is empty
        let temp_dir = TempDir::new("openssl_empty_cert");
        let test_dir_path = temp_dir.path();
        let cert_path = test_dir_path.join("empty_cert.pem");
        fs::write(&cert_path, "").unwrap();
        let yaml = YamlLoader::load_from_str(&format!("|-\n  {}", cert_path.display())).unwrap();
        assert!(as_openssl_certificates(&yaml[0], Some(test_dir_path)).is_err());
    }

    #[test]
    fn as_openssl_private_key_ok() {
        let temp_dir = TempDir::new("openssl_ok");
        let test_dir_path = temp_dir.path();
        let key_path = test_dir_path.join("test_key.pem");
        fs::write(&key_path, TEST_KEY_PEM1).unwrap();

        // string value (PEM)
        let yaml = Yaml::String(TEST_KEY_PEM1.to_string());
        let key = as_openssl_private_key(&yaml, None).unwrap();
        assert!(key.private_key_to_pem_pkcs8().is_ok());

        // file path
        let yaml = YamlLoader::load_from_str(&format!("|-\n  {}", key_path.display())).unwrap();
        let key = as_openssl_private_key(&yaml[0], Some(test_dir_path)).unwrap();
        assert!(key.private_key_to_pem_pkcs8().is_ok());
    }

    #[test]
    fn as_openssl_private_key_err() {
        // invalid string value
        let yaml = yaml_str!("invalid_key");
        assert!(as_openssl_private_key(&yaml, None).is_err());

        // non-existent file
        let yaml = yaml_str!("non_existent_file.pem");
        assert!(as_openssl_private_key(&yaml, Some(Path::new("/non_existent_dir"))).is_err());
    }

    #[test]
    fn as_openssl_certificate_pair_ok() {
        let temp_dir = TempDir::new("openssl_ok");
        let test_dir_path = temp_dir.path();
        let cert_path = test_dir_path.join("test_cert.pem");
        fs::write(&cert_path, TEST_CERT_PEM1).unwrap();
        let key_path = test_dir_path.join("test_key.pem");
        fs::write(&key_path, TEST_KEY_PEM1).unwrap();

        let yaml = YamlLoader::load_from_str(&format!(
            r#"
                certificate: |-
                    {}
                private_key: |-
                    {}
            "#,
            cert_path.display(),
            key_path.display()
        ))
        .unwrap();
        let pair = as_openssl_certificate_pair(&yaml[0], Some(test_dir_path)).unwrap();
        assert!(pair.is_set());

        let yaml = YamlLoader::load_from_str(&format!(
            r#"
                cert: |-
                    {}
                key: |-
                    {}
            "#,
            cert_path.display(),
            key_path.display()
        ))
        .unwrap();
        let pair = as_openssl_certificate_pair(&yaml[0], Some(test_dir_path)).unwrap();
        assert!(pair.is_set());
    }

    #[test]
    fn as_openssl_certificate_pair_err() {
        // invalid YAML type
        let yaml = yaml_str!("invalid");
        assert!(as_openssl_certificate_pair(&yaml, None).is_err());

        let yaml = Yaml::Boolean(false);
        assert!(as_openssl_certificate_pair(&yaml, None).is_err());

        // empty map
        let yaml = yaml_doc!(r#"{}"#);
        assert!(as_openssl_certificate_pair(&yaml, None).is_err());

        // invalid certificate and key
        let yaml = yaml_doc!(
            r#"
            certificate: "invalid_cert"
            private_key: "invalid_key"
        "#
        );
        assert!(as_openssl_certificate_pair(&yaml, None).is_err());

        // unknown key
        let yaml = yaml_doc!(
            r#"
            unknown_key: "value"
        "#
        );
        assert!(as_openssl_certificate_pair(&yaml, None).is_err());
    }

    #[test]
    fn as_openssl_tlcp_certificate_pair_ok() {
        let temp_dir = TempDir::new("openssl_ok");
        let test_dir_path = temp_dir.path();
        let sign_cert_path = test_dir_path.join("sign_cert.pem");
        fs::write(&sign_cert_path, TEST_CERT_PEM1).unwrap();
        let enc_cert_path = test_dir_path.join("enc_cert.pem");
        fs::write(&enc_cert_path, TEST_CERT_PEM2).unwrap();
        let sign_key_path = test_dir_path.join("sign_key.pem");
        fs::write(&sign_key_path, TEST_KEY_PEM1).unwrap();
        let enc_key_path = test_dir_path.join("enc_key.pem");
        fs::write(&enc_key_path, TEST_KEY_PEM2).unwrap();

        let yaml = YamlLoader::load_from_str(&format!(
            r#"
                sign_certificate: |-
                    {}
                enc_certificate: |-
                    {}
                sign_private_key: |-
                    {}
                enc_private_key: |-
                    {}
            "#,
            sign_cert_path.display(),
            enc_cert_path.display(),
            sign_key_path.display(),
            enc_key_path.display()
        ))
        .unwrap();
        let pair = as_openssl_tlcp_certificate_pair(&yaml[0], Some(test_dir_path)).unwrap();
        assert!(pair.check().is_ok());
    }

    #[test]
    fn as_openssl_tlcp_certificate_pair_err() {
        // invalid YAML type
        let yaml = yaml_str!("invalid");
        assert!(as_openssl_tlcp_certificate_pair(&yaml, None).is_err());

        let yaml = Yaml::Null;
        assert!(as_openssl_tlcp_certificate_pair(&yaml, None).is_err());

        // empty map
        let yaml = yaml_doc!(r#"{}"#);
        assert!(as_openssl_tlcp_certificate_pair(&yaml, None).is_err());

        // invalid certificates and keys
        let yaml = yaml_doc!(
            r#"
            sign_certificate: "invalid_cert"
            enc_certificate: "invalid_cert"
            sign_private_key: "invalid_key"
            enc_private_key: "invalid_key"
        "#
        );
        assert!(as_openssl_tlcp_certificate_pair(&yaml, None).is_err());

        // unknown key
        let yaml = yaml_doc!(
            r#"
            unknown_key: "value"
        "#
        );
        assert!(as_openssl_tlcp_certificate_pair(&yaml, None).is_err());
    }

    #[test]
    fn as_to_one_openssl_tls_client_config_builder_ok() {
        let temp_dir = TempDir::new("openssl_ok");
        let test_dir_path = temp_dir.path();
        let cert_path = test_dir_path.join("test_cert.pem");
        fs::write(&cert_path, TEST_CERT_PEM1).unwrap();
        let key_path = test_dir_path.join("test_key.pem");
        fs::write(&key_path, TEST_KEY_PEM1).unwrap();
        let sign_cert_path = test_dir_path.join("sign_cert.pem");
        fs::write(&sign_cert_path, TEST_CERT_PEM1).unwrap();
        let enc_cert_path = test_dir_path.join("enc_cert.pem");
        fs::write(&enc_cert_path, TEST_CERT_PEM2).unwrap();
        let sign_key_path = test_dir_path.join("sign_key.pem");
        fs::write(&sign_key_path, TEST_KEY_PEM1).unwrap();
        let enc_key_path = test_dir_path.join("enc_key.pem");
        fs::write(&enc_key_path, TEST_KEY_PEM2).unwrap();
        let yaml = YamlLoader::load_from_str(&format!(
            r#"
                cert: |-
                    {}
                key: |-
                    {}
            "#,
            cert_path.display(),
            key_path.display()
        ))
        .unwrap();
        let cert_pair = as_openssl_certificate_pair(&yaml[0], Some(test_dir_path)).unwrap();
        let yaml = YamlLoader::load_from_str(&format!(
            r#"
                sign_certificate: |-
                    {}
                enc_certificate: |-
                    {}
                sign_private_key: |-
                    {}
                enc_private_key: |-
                    {}
            "#,
            sign_cert_path.display(),
            enc_cert_path.display(),
            sign_key_path.display(),
            enc_key_path.display()
        ))
        .unwrap();
        let tlcp_cert_pair =
            as_openssl_tlcp_certificate_pair(&yaml[0], Some(test_dir_path)).unwrap();

        let yaml = YamlLoader::load_from_str(&format!(
            r#"
                protocol: "tls12"
                min_tls_version: "tls1.2"
                max_tls_version: "tls1.3"
                ciphers: ["TLS_AES_128_GCM_SHA256"]
                disable_sni: true
                cert_pair:
                  certificate: |-
                    {}
                  private_key: |-
                    {}
                tlcp_cert_pair:
                  sign_certificate: |-
                    {}
                  enc_certificate: |-
                    {}
                  sign_private_key: |-
                    {}
                  enc_private_key: |-
                    {}
                ca_certificate: |-
                    {}
                no_default_ca_certificate: true
                handshake_timeout: "10s"
                no_session_cache: true
                use_builtin_session_cache: true
                session_cache_lru_max_sites: 100
                session_cache_each_capacity: 10
                supported_groups: "P-256"
                use_ocsp_stapling: true
                enable_sct: true
                enable_grease: true
                permute_extensions: true
                insecure: false
            "#,
            cert_path.display(),
            key_path.display(),
            sign_cert_path.display(),
            enc_cert_path.display(),
            sign_key_path.display(),
            enc_key_path.display(),
            cert_path.display()
        ))
        .unwrap();
        let builder =
            as_to_one_openssl_tls_client_config_builder(&yaml[0], Some(test_dir_path)).unwrap();
        let mut expected = OpensslClientConfigBuilder::default();
        expected.set_protocol(OpensslProtocol::Tls12);
        expected.set_min_tls_version(TlsVersion::TLS1_2);
        expected.set_max_tls_version(TlsVersion::TLS1_3);
        expected.set_ciphers(vec!["TLS_AES_128_GCM_SHA256".to_string()]);
        expected.set_disable_sni();
        expected.set_cert_pair(cert_pair);
        expected.set_tlcp_cert_pair(tlcp_cert_pair);
        let ca_cert = X509::from_pem(TEST_CERT_PEM1.as_bytes()).unwrap();
        expected.set_ca_certificates(vec![ca_cert]).unwrap();
        expected.set_no_default_ca_certificates();
        expected.set_handshake_timeout(Duration::from_secs(10));
        expected.set_no_session_cache();
        expected.set_use_builtin_session_cache();
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
        // invalid YAML type
        let yaml = yaml_str!("invalid");
        assert!(as_to_one_openssl_tls_client_config_builder(&yaml, None).is_err());

        // non-string yaml type value for protocol
        let yaml = yaml_doc!(
            r#"
            protocol: 123
            "#
        );
        assert!(as_to_one_openssl_tls_client_config_builder(&yaml, None).is_err());

        // invalid protocol
        let yaml = yaml_doc!(
            r#"
            protocol: "invalid_protocol"
        "#
        );
        assert!(as_to_one_openssl_tls_client_config_builder(&yaml, None).is_err());

        // invalid min_tls_version
        let yaml = yaml_doc!(
            r#"
            min_tls_version: "invalid_version"
        "#
        );
        assert!(as_to_one_openssl_tls_client_config_builder(&yaml, None).is_err());

        // unknown key
        let yaml = yaml_doc!(
            r#"
            unknown_key: "value"
        "#
        );
        assert!(as_to_one_openssl_tls_client_config_builder(&yaml, None).is_err());

        // duplicate certificate config
        let temp_dir = TempDir::new("openssl_duplicate_cert");
        let test_dir_path = temp_dir.path();
        let cert_path = test_dir_path.join("test_cert.pem");
        fs::write(&cert_path, TEST_CERT_PEM1).unwrap();
        let key_path = test_dir_path.join("test_key.pem");
        fs::write(&key_path, TEST_KEY_PEM1).unwrap();
        let yaml = YamlLoader::load_from_str(&format!(
            r#"
            cert: |-
                {}
            key: |-
                {}
            cert_pair:
              certificate: |-
                {}
              private_key: |-
                {}
            "#,
            cert_path.display(),
            key_path.display(),
            cert_path.display(),
            key_path.display()
        ))
        .unwrap();
        assert!(
            as_to_one_openssl_tls_client_config_builder(&yaml[0], Some(test_dir_path)).is_err()
        );
    }

    #[test]
    fn as_to_many_openssl_tls_client_config_builder_ok() {
        let temp_dir = TempDir::new("openssl_ok");
        let test_dir_path = temp_dir.path();
        let cert_path = test_dir_path.join("test_cert.pem");
        fs::write(&cert_path, TEST_CERT_PEM1).unwrap();
        let key_path = test_dir_path.join("test_key.pem");
        fs::write(&key_path, TEST_KEY_PEM1).unwrap();
        let sign_cert_path = test_dir_path.join("sign_cert.pem");
        fs::write(&sign_cert_path, TEST_CERT_PEM1).unwrap();
        let enc_cert_path = test_dir_path.join("enc_cert.pem");
        fs::write(&enc_cert_path, TEST_CERT_PEM2).unwrap();
        let sign_key_path = test_dir_path.join("sign_key.pem");
        fs::write(&sign_key_path, TEST_KEY_PEM1).unwrap();
        let enc_key_path = test_dir_path.join("enc_key.pem");
        fs::write(&enc_key_path, TEST_KEY_PEM2).unwrap();
        let yaml = YamlLoader::load_from_str(&format!(
            r#"
                cert: |-
                    {}
                key: |-
                    {}
            "#,
            cert_path.display(),
            key_path.display()
        ))
        .unwrap();
        let cert_pair = as_openssl_certificate_pair(&yaml[0], Some(test_dir_path)).unwrap();
        let yaml = YamlLoader::load_from_str(&format!(
            r#"
                sign_certificate: |-
                    {}
                enc_certificate: |-
                    {}
                sign_private_key: |-
                    {}
                enc_private_key: |-
                    {}
            "#,
            sign_cert_path.display(),
            enc_cert_path.display(),
            sign_key_path.display(),
            enc_key_path.display()
        ))
        .unwrap();
        let tlcp_cert_pair =
            as_openssl_tlcp_certificate_pair(&yaml[0], Some(test_dir_path)).unwrap();

        let yaml = YamlLoader::load_from_str(&format!(
            r#"
                protocol: "tls12"
                tls_version_min: "tls1.2"
                tls_version_max: "tls1.3"
                ciphers: "TLS_AES_128_GCM_SHA256"
                disable_sni: true
                cert: |-
                    {}
                key: |-
                    {}
                tlcp_cert_pair:
                  sign_certificate: |-
                    {}
                  enc_certificate: |-
                    {}
                  sign_private_key: |-
                    {}
                  enc_private_key: |-
                    {}
                ca_certificate: |-
                    {}
                no_default_ca_cert: true
                negotiation_timeout: "10s"
                disable_session_cache: true
                use_builtin_session_cache: false
                session_cache_lru_max_sites: 100
                session_cache_each_cap: 10
                supported_groups: "P-256"
                use_ocsp_stapling: true
                enable_sct: true
                enable_grease: true
                permute_extensions: true
                insecure: false
            "#,
            cert_path.display(),
            key_path.display(),
            sign_cert_path.display(),
            enc_cert_path.display(),
            sign_key_path.display(),
            enc_key_path.display(),
            cert_path.display()
        ))
        .unwrap();
        let builder =
            as_to_many_openssl_tls_client_config_builder(&yaml[0], Some(test_dir_path)).unwrap();
        let mut expected = OpensslClientConfigBuilder::default();
        expected.set_protocol(OpensslProtocol::Tls12);
        expected.set_min_tls_version(TlsVersion::TLS1_2);
        expected.set_max_tls_version(TlsVersion::TLS1_3);
        expected.set_ciphers(vec!["TLS_AES_128_GCM_SHA256".to_string()]);
        expected.set_disable_sni();
        expected.set_cert_pair(cert_pair);
        expected.set_tlcp_cert_pair(tlcp_cert_pair);
        let ca_cert = X509::from_pem(TEST_CERT_PEM1.as_bytes()).unwrap();
        expected.set_ca_certificates(vec![ca_cert]).unwrap();
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
    fn as_to_many_openssl_tls_client_config_builder_err() {
        // invalid YAML type
        let yaml = yaml_str!("invalid");
        assert!(as_to_many_openssl_tls_client_config_builder(&yaml, None).is_err());

        // invalid cipher string
        let yaml = yaml_doc!(
            r#"
            ciphers: ["invalid cipher"]
            "#
        );
        assert!(as_to_many_openssl_tls_client_config_builder(&yaml, None).is_err());

        // invalid yaml type for ciphers
        let yaml = yaml_doc!(
            r#"
            ciphers: 123
            "#
        );
        assert!(as_to_many_openssl_tls_client_config_builder(&yaml, None).is_err());

        // invalid protocol
        let yaml = yaml_doc!(
            r#"
            protocol: "invalid_protocol"
        "#
        );
        assert!(as_to_many_openssl_tls_client_config_builder(&yaml, None).is_err());

        // invalid min_tls_version
        let yaml = yaml_doc!(
            r#"
            min_tls_version: "invalid_version"
        "#
        );
        assert!(as_to_many_openssl_tls_client_config_builder(&yaml, None).is_err());

        // unknown key
        let yaml = yaml_doc!(
            r#"
            unknown_key: "value"
        "#
        );
        assert!(as_to_many_openssl_tls_client_config_builder(&yaml, None).is_err());
    }

    #[test]
    fn as_tls_interception_client_config_builder_ok() {
        let temp_dir = TempDir::new("openssl_ok");
        let test_dir_path = temp_dir.path();
        let cert_path = test_dir_path.join("test_cert.pem");
        fs::write(&cert_path, TEST_CERT_PEM1).unwrap();

        let yaml = YamlLoader::load_from_str(&format!(
            r#"
                min_tls_version: "tls1.2"
                max_tls_version: "tls1.3"
                ca_certificate: |-
                    {}
                no_default_ca_certificate: true
                handshake_timeout: "10s"
                no_session_cache: true
                session_cache_lru_max_sites: 100
                session_cache_each_capacity: 10
                supported_groups: "P-256"
                use_ocsp_stapling: true
                enable_sct: true
                enable_grease: true
                permute_extensions: true
                insecure: false
            "#,
            cert_path.display()
        ))
        .unwrap();
        let builder =
            as_tls_interception_client_config_builder(&yaml[0], Some(test_dir_path)).unwrap();
        let mut expected = OpensslInterceptionClientConfigBuilder::default();
        expected.set_min_tls_version(TlsVersion::TLS1_2);
        expected.set_max_tls_version(TlsVersion::TLS1_3);
        let ca_cert = X509::from_pem(TEST_CERT_PEM1.as_bytes()).unwrap();
        expected.set_ca_certificates(vec![ca_cert]).unwrap();
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

        // alias keys
        let yaml = YamlLoader::load_from_str(&format!(
            r#"
                tls_version_min: "tls1.2"
                tls_version_max: "tls1.3"
                ca_cert: |-
                    {}
                no_default_ca_cert: true
                negotiation_timeout: "10s"
                disable_session_cache: true
                session_cache_each_cap: 10
            "#,
            cert_path.display()
        ))
        .unwrap();
        let builder =
            as_tls_interception_client_config_builder(&yaml[0], Some(test_dir_path)).unwrap();
        let mut expected = OpensslInterceptionClientConfigBuilder::default();
        expected.set_min_tls_version(TlsVersion::TLS1_2);
        expected.set_max_tls_version(TlsVersion::TLS1_3);
        let ca_cert = X509::from_pem(TEST_CERT_PEM1.as_bytes()).unwrap();
        expected.set_ca_certificates(vec![ca_cert]).unwrap();
        expected.set_no_default_ca_certificates();
        expected.set_handshake_timeout(Duration::from_secs(10));
        expected.set_no_session_cache();
        expected.set_session_cache_each_capacity(10);
        assert_eq!(builder, expected);
    }

    #[test]
    fn as_tls_interception_client_config_builder_err() {
        // invalid YAML type
        let yaml = yaml_str!("invalid");
        assert!(as_tls_interception_client_config_builder(&yaml, None).is_err());

        let yaml = Yaml::Boolean(false);
        assert!(as_tls_interception_client_config_builder(&yaml, None).is_err());

        // invalid min_tls_version
        let yaml = yaml_doc!(
            r#"
            min_tls_version: "invalid_version"
        "#
        );
        assert!(as_tls_interception_client_config_builder(&yaml, None).is_err());

        // unknown key
        let yaml = yaml_doc!(
            r#"
            unknown_key: "value"
        "#
        );
        assert!(as_tls_interception_client_config_builder(&yaml, None).is_err());
    }

    #[test]
    fn as_openssl_tls_server_config_builder_ok() {
        let temp_dir = TempDir::new("openssl_ok");
        let test_dir_path = temp_dir.path();
        let cert_path = test_dir_path.join("test_cert.pem");
        fs::write(&cert_path, TEST_CERT_PEM1).unwrap();
        let key_path = test_dir_path.join("test_key.pem");
        fs::write(&key_path, TEST_KEY_PEM1).unwrap();
        let sign_cert_path = test_dir_path.join("sign_cert.pem");
        fs::write(&sign_cert_path, TEST_CERT_PEM1).unwrap();
        let enc_cert_path = test_dir_path.join("enc_cert.pem");
        fs::write(&enc_cert_path, TEST_CERT_PEM2).unwrap();
        let sign_key_path = test_dir_path.join("sign_key.pem");
        fs::write(&sign_key_path, TEST_KEY_PEM1).unwrap();
        let enc_key_path = test_dir_path.join("enc_key.pem");
        fs::write(&enc_key_path, TEST_KEY_PEM2).unwrap();
        let yaml = YamlLoader::load_from_str(&format!(
            r#"
                cert: |-
                    {}
                key: |-
                    {}
            "#,
            cert_path.display(),
            key_path.display()
        ))
        .unwrap();
        let cert_pair1 = as_openssl_certificate_pair(&yaml[0], Some(test_dir_path)).unwrap();
        let cert_pair2 = as_openssl_certificate_pair(&yaml[0], Some(test_dir_path)).unwrap();
        let yaml = YamlLoader::load_from_str(&format!(
            r#"
                sign_certificate: |-
                    {}
                enc_certificate: |-
                    {}
                sign_private_key: |-
                    {}
                enc_private_key: |-
                    {}
            "#,
            sign_cert_path.display(),
            enc_cert_path.display(),
            sign_key_path.display(),
            enc_key_path.display()
        ))
        .unwrap();
        let tlcp_cert_pair1 =
            as_openssl_tlcp_certificate_pair(&yaml[0], Some(test_dir_path)).unwrap();
        let tlcp_cert_pair2 =
            as_openssl_tlcp_certificate_pair(&yaml[0], Some(test_dir_path)).unwrap();

        let yaml = YamlLoader::load_from_str(&format!(
            r#"
                cert_pairs:
                  certificate: |-
                    {}
                  private_key: |-
                    {}
                tlcp_cert_pairs:
                  sign_certificate: |-
                    {}
                  enc_certificate: |-
                    {}
                  sign_private_key: |-
                    {}
                  enc_private_key: |-
                    {}
                enable_client_auth: true
                session_id_context: "test_session_id_context"
                no_session_ticket: true
                no_session_cache: true
                ca_certificate: |-
                    {}
                handshake_timeout: "10s"
            "#,
            cert_path.display(),
            key_path.display(),
            sign_cert_path.display(),
            enc_cert_path.display(),
            sign_key_path.display(),
            enc_key_path.display(),
            cert_path.display()
        ))
        .unwrap();
        let builder = as_openssl_tls_server_config_builder(&yaml[0], Some(test_dir_path)).unwrap();
        let mut expected = OpensslServerConfigBuilder::empty();
        expected.push_cert_pair(cert_pair1).unwrap();
        expected.push_tlcp_cert_pair(tlcp_cert_pair1).unwrap();
        expected.enable_client_auth();
        expected.set_session_id_context("test_session_id_context".to_string());
        expected.set_disable_session_ticket(true);
        expected.set_disable_session_cache(true);
        let ca_cert = X509::from_pem(TEST_CERT_PEM1.as_bytes()).unwrap();
        expected
            .set_client_auth_certificates(vec![ca_cert])
            .unwrap();
        expected.set_accept_timeout(Duration::from_secs(10));
        assert_eq!(builder, expected);

        // array yaml type for cert_pairs and tlcp_cert_pairs
        let yaml = YamlLoader::load_from_str(&format!(
            r#"
                cert_pairs:
                  - certificate: |-
                        {}
                    private_key: |-
                        {}
                tlcp_cert_pairs:
                  - sign_certificate: |-
                        {}
                    enc_certificate: |-
                        {}
                    sign_private_key: |-
                        {}
                    enc_private_key: |-
                        {}
            "#,
            cert_path.display(),
            key_path.display(),
            sign_cert_path.display(),
            enc_cert_path.display(),
            sign_key_path.display(),
            enc_key_path.display()
        ))
        .unwrap();
        let builder = as_openssl_tls_server_config_builder(&yaml[0], Some(test_dir_path)).unwrap();
        let mut expected = OpensslServerConfigBuilder::empty();
        expected.push_cert_pair(cert_pair2).unwrap();
        expected.push_tlcp_cert_pair(tlcp_cert_pair2).unwrap();
        assert_eq!(builder, expected);
    }

    #[test]
    fn as_openssl_tls_server_config_builder_err() {
        // invalid YAML type
        let yaml = yaml_str!("invalid");
        assert!(as_openssl_tls_server_config_builder(&yaml, None).is_err());

        let yaml = Yaml::Null;
        assert!(as_openssl_tls_server_config_builder(&yaml, None).is_err());

        // invalid cert_pairs
        let yaml = yaml_doc!(
            r#"
            cert_pairs: "invalid"
        "#
        );
        assert!(as_openssl_tls_server_config_builder(&yaml, None).is_err());

        let yaml = yaml_doc!(
            r#"
            cert_pairs:
              certificate: "invalid"
              private_key: "invalid"
        "#
        );
        assert!(as_openssl_tls_server_config_builder(&yaml, None).is_err());

        // invalid tlcp_cert_pairs
        let yaml = yaml_doc!(
            r#"
            tlcp_cert_pairs: "invalid"
        "#
        );
        assert!(as_openssl_tls_server_config_builder(&yaml, None).is_err());

        let yaml = yaml_doc!(
            r#"
            tlcp_cert_pairs:
              sign_certificate: "invalid"
              enc_certificate: "invalid"
              sign_private_key: "invalid"
              enc_private_key: "invalid"
        "#
        );
        assert!(as_openssl_tls_server_config_builder(&yaml, None).is_err());

        // unknown key
        let yaml = yaml_doc!(
            r#"
            unknown_key: "value"
        "#
        );
        assert!(as_openssl_tls_server_config_builder(&yaml, None).is_err());
    }

    #[test]
    fn as_tls_interception_server_config_builder_ok() {
        let yaml = yaml_doc!(
            r#"
            handshake_timeout: "10s"
        "#
        );
        let builder = as_tls_interception_server_config_builder(&yaml).unwrap();
        let mut expected = OpensslInterceptionServerConfigBuilder::default();
        expected.set_accept_timeout(Duration::from_secs(10));
        assert_eq!(builder, expected);

        let yaml = yaml_doc!(
            r#"
            negotiation_timeout: "20s"
        "#
        );
        let builder = as_tls_interception_server_config_builder(&yaml).unwrap();
        let mut expected = OpensslInterceptionServerConfigBuilder::default();
        expected.set_accept_timeout(Duration::from_secs(20));
        assert_eq!(builder, expected);

        let yaml = yaml_doc!(
            r#"
            accept_timeout: "30s"
        "#
        );
        let builder = as_tls_interception_server_config_builder(&yaml).unwrap();
        let mut expected = OpensslInterceptionServerConfigBuilder::default();
        expected.set_accept_timeout(Duration::from_secs(30));
        assert_eq!(builder, expected);
    }

    #[test]
    fn as_tls_interception_server_config_builder_err() {
        // invalid YAML type
        let yaml = yaml_str!("invalid");
        assert!(as_tls_interception_server_config_builder(&yaml).is_err());

        let yaml = Yaml::Integer(123);
        assert!(as_tls_interception_server_config_builder(&yaml).is_err());

        // unknown key
        let yaml = yaml_doc!(
            r#"
            unknown_key: "value"
        "#
        );
        assert!(as_tls_interception_server_config_builder(&yaml).is_err());

        // invalid value
        let yaml = yaml_doc!(
            r#"
            handshake_timeout: "invalid_value"
        "#
        );
        assert!(as_tls_interception_server_config_builder(&yaml).is_err());
    }
}
