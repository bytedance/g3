/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::path::Path;

use anyhow::{Context, anyhow};
use rustls_pki_types::pem::PemObject;
use rustls_pki_types::{CertificateDer, PrivateKeyDer, ServerName};
use yaml_rust::Yaml;

use g3_types::net::{
    RustlsCertificatePair, RustlsCertificatePairBuilder, RustlsClientConfigBuilder,
    RustlsServerConfigBuilder,
};

pub fn as_rustls_server_name(value: &Yaml) -> anyhow::Result<ServerName<'static>> {
    if let Yaml::String(s) = value {
        ServerName::try_from(s.as_str())
            .map(|r| r.to_owned())
            .map_err(|e| anyhow!("invalid rustls server name string: {e}"))
    } else {
        Err(anyhow!(
            "yaml value type for 'rustls server name' should be 'string'"
        ))
    }
}

fn as_certificates_from_single_element(
    value: &Yaml,
    lookup_dir: Option<&Path>,
) -> anyhow::Result<Vec<CertificateDer<'static>>> {
    let mut certs = Vec::new();
    if let Yaml::String(s) = value {
        if s.trim_start().starts_with("--") {
            for (i, r) in CertificateDer::pem_slice_iter(s.as_bytes()).enumerate() {
                let cert = r.map_err(|e| anyhow!("invalid certificate #{i}: {e:?}"))?;
                certs.push(cert);
            }
            return if certs.is_empty() {
                Err(anyhow!("no valid certificate found"))
            } else {
                Ok(certs)
            };
        }
    }

    let (file, path) = crate::value::as_file(value, lookup_dir).context("invalid file")?;
    for (i, r) in CertificateDer::pem_reader_iter(file).enumerate() {
        let cert = r.map_err(|e| anyhow!("invalid certificate {}#{i}: {e:?}", path.display()))?;
        certs.push(cert);
    }
    if certs.is_empty() {
        Err(anyhow!("no valid certificate found"))
    } else {
        Ok(certs)
    }
}

pub fn as_rustls_certificates(
    value: &Yaml,
    lookup_dir: Option<&Path>,
) -> anyhow::Result<Vec<CertificateDer<'static>>> {
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

pub fn as_rustls_private_key(
    value: &Yaml,
    lookup_dir: Option<&Path>,
) -> anyhow::Result<PrivateKeyDer<'static>> {
    if let Yaml::String(s) = value {
        if s.trim_start().starts_with("--") {
            return PrivateKeyDer::from_pem_slice(s.as_bytes())
                .map_err(|e| anyhow!("invalid private key string: {e:?}"));
        }
    }

    let (file, path) = crate::value::as_file(value, lookup_dir).context("invalid file")?;
    PrivateKeyDer::from_pem_reader(file)
        .map_err(|e| anyhow!("invalid private key file {}: {e:?}", path.display()))
}

pub fn as_rustls_certificate_pair(
    value: &Yaml,
    lookup_dir: Option<&Path>,
) -> anyhow::Result<RustlsCertificatePair> {
    if let Yaml::Hash(map) = value {
        let mut pair_builder = RustlsCertificatePairBuilder::default();
        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "certificate" | "cert" => {
                let certs = as_rustls_certificates(v, lookup_dir)
                    .context(format!("invalid certificates value for key {k}"))?;
                pair_builder.set_certs(certs);
                Ok(())
            }
            "private_key" | "key" => {
                let key = as_rustls_private_key(v, lookup_dir)
                    .context(format!("invalid private key value for key {k}"))?;
                pair_builder.set_key(key);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;
        pair_builder.build()
    } else {
        Err(anyhow!(
            "yaml value type for rustls certificate pair should be 'map'"
        ))
    }
}

pub fn as_rustls_client_config_builder(
    value: &Yaml,
    lookup_dir: Option<&Path>,
) -> anyhow::Result<RustlsClientConfigBuilder> {
    if let Yaml::Hash(map) = value {
        let mut builder = RustlsClientConfigBuilder::default();
        let mut cert_pair_builder = RustlsCertificatePairBuilder::default();

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "no_session_cache" | "disable_session_cache" | "session_cache_disabled" => {
                let no =
                    crate::value::as_bool(v).context(format!("invalid bool value for key {k}"))?;
                if no {
                    builder.set_no_session_cache();
                }
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
            "max_fragment_size" => {
                let mtu = crate::value::as_usize(v)
                    .context(format!("invalid usize value for key {k}"))?;
                builder.set_max_fragment_size(mtu);
                Ok(())
            }
            "certificate" | "cert" => {
                let certs = as_rustls_certificates(v, lookup_dir)
                    .context(format!("invalid certificates value for key {k}"))?;
                cert_pair_builder.set_certs(certs);
                Ok(())
            }
            "private_key" | "key" => {
                let key = as_rustls_private_key(v, lookup_dir)
                    .context(format!("invalid private key value for key {k}"))?;
                cert_pair_builder.set_key(key);
                Ok(())
            }
            "cert_pair" => {
                let pair = as_rustls_certificate_pair(v, lookup_dir)
                    .context(format!("invalid cert pair value for key {k}"))?;
                builder.set_cert_pair(pair);
                Ok(())
            }
            "ca_certificate" | "ca_cert" | "server_auth_certificate" | "server_auth_cert" => {
                let certs = as_rustls_certificates(v, lookup_dir)
                    .context(format!("invalid certificates value for key {k}"))?;
                builder.set_ca_certificates(certs);
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
            "use_builtin_ca_certificate" | "use_builtin_ca_cert" => {
                let use_builtin =
                    crate::value::as_bool(v).context(format!("invalid bool value for key {k}"))?;
                if use_builtin {
                    builder.set_use_builtin_ca_certificates();
                }
                Ok(())
            }
            "handshake_timeout" | "negotiation_timeout" => {
                let timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                builder.set_negotiation_timeout(timeout);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        if let Ok(cert_pair) = cert_pair_builder.build() {
            if builder.set_cert_pair(cert_pair).is_some() {
                return Err(anyhow!("found duplicate client certificate config"));
            }
        }

        builder.check()?;
        Ok(builder)
    } else {
        Err(anyhow!(
            "yaml value type for 'rustls client config builder' should be 'map'"
        ))
    }
}

pub fn as_rustls_server_config_builder(
    value: &Yaml,
    lookup_dir: Option<&Path>,
) -> anyhow::Result<RustlsServerConfigBuilder> {
    if let Yaml::Hash(map) = value {
        let mut builder = RustlsServerConfigBuilder::empty();
        let mut cert_pair_builder = RustlsCertificatePairBuilder::default();

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "cert_pairs" => {
                if let Yaml::Array(seq) = v {
                    for (i, v) in seq.iter().enumerate() {
                        let pair = as_rustls_certificate_pair(v, lookup_dir)
                            .context(format!("invalid rustls cert pair value for {k}#{i}"))?;
                        builder.push_cert_pair(pair);
                    }
                } else {
                    let pair = as_rustls_certificate_pair(v, lookup_dir)
                        .context(format!("invalid rustls cert pair value for key {k}"))?;
                    builder.push_cert_pair(pair);
                }
                Ok(())
            }
            "certificate" | "cert" => {
                let certs = as_rustls_certificates(v, lookup_dir)
                    .context(format!("invalid value for key {k}"))?;
                cert_pair_builder.set_certs(certs);
                Ok(())
            }
            "private_key" | "key" => {
                let key = as_rustls_private_key(v, lookup_dir)
                    .context(format!("invalid value for key {k}"))?;
                cert_pair_builder.set_key(key);
                Ok(())
            }
            "enable_client_auth" => {
                let enable = crate::value::as_bool(v)?;
                if enable {
                    builder.enable_client_auth();
                }
                Ok(())
            }
            "use_session_ticket" => {
                let enable = crate::value::as_bool(v)?;
                builder.set_use_session_ticket(enable);
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
                let certs = as_rustls_certificates(v, lookup_dir)
                    .context(format!("invalid value for key {k}"))?;
                builder.set_client_auth_certificates(certs);
                Ok(())
            }
            "handshake_timeout" | "negotiation_timeout" | "accept_timeout" => {
                let timeout = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                builder.set_accept_timeout(timeout);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        if let Ok(cert_pair) = cert_pair_builder.build() {
            builder.push_cert_pair(cert_pair);
        }

        builder.check()?;
        Ok(builder)
    } else {
        Err(anyhow!(
            "yaml value type for 'rustls server config builder' should be 'map'"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use yaml_rust::YamlLoader;

    const TEST_CERT_PEM: &str = include_str!("./test_data/test_cert1.pem");
    const TEST_KEY_PEM: &str = include_str!("./test_data/test_key1.pem");

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
    fn as_rustls_server_name_ok() {
        // valid DNS name
        let yaml = yaml_str!("example.com");
        assert_eq!(
            as_rustls_server_name(&yaml).unwrap(),
            ServerName::try_from("example.com").unwrap()
        );

        // valid IP address
        let yaml = yaml_str!("192.168.0.1");
        assert_eq!(
            as_rustls_server_name(&yaml).unwrap(),
            ServerName::try_from("192.168.0.1").unwrap()
        );
    }

    #[test]
    fn as_rustls_server_name_err() {
        // non-string YAML
        let yaml = Yaml::Integer(123);
        assert!(as_rustls_server_name(&yaml).is_err());

        // empty string
        let yaml = yaml_str!("");
        assert!(as_rustls_server_name(&yaml).is_err());

        // invalid DNS name
        let yaml = yaml_str!("invalid domain");
        assert!(as_rustls_server_name(&yaml).is_err());

        // invalid IP address
        let yaml = yaml_str!("192.168.0.256");
        assert!(as_rustls_server_name(&yaml).is_err());
    }

    #[test]
    fn as_rustls_certificates_ok() {
        let temp_dir = TempDir::new("rustls_cert_ok");
        let test_dir_path = temp_dir.path();
        let cert_path = test_dir_path.join("test_cert.pem");
        fs::write(&cert_path, TEST_CERT_PEM).unwrap();

        // single PEM string
        let yaml = Yaml::String(TEST_CERT_PEM.to_string());
        let certs = as_rustls_certificates(&yaml, None).unwrap();
        assert!(!certs.is_empty());

        // file path
        let yaml = YamlLoader::load_from_str(&format!("|-\n  {}", cert_path.display())).unwrap();
        let certs = as_rustls_certificates(&yaml[0], Some(temp_dir.path())).unwrap();
        assert!(!certs.is_empty());

        // array of PEM strings
        let yaml = Yaml::Array(vec![
            Yaml::String(TEST_CERT_PEM.to_string()),
            Yaml::String(TEST_CERT_PEM.to_string()),
        ]);
        let certs = as_rustls_certificates(&yaml, None).unwrap();
        assert_eq!(certs.len(), 2);
    }

    #[test]
    fn as_rustls_certificates_err() {
        // invalid PEM string
        let yaml = yaml_str!("invalid cert");
        assert!(as_rustls_certificates(&yaml, None).is_err());

        // non-existent file
        let yaml = yaml_str!("non_existent_cert.pem");
        assert!(as_rustls_certificates(&yaml, Some(Path::new("/non_existent"))).is_err());

        // empty array
        let yaml = yaml_doc!(r#"- []"#);
        assert!(as_rustls_certificates(&yaml, None).is_err());

        // array with invalid PEM
        let yaml = Yaml::Array(vec![
            Yaml::String("invalid cert".to_string()),
            Yaml::String(TEST_CERT_PEM.to_string()),
        ]);
        assert!(as_rustls_certificates(&yaml, None).is_err());

        // empty certificate
        let temp_dir = TempDir::new("rustls_cert_err");
        let test_dir_path = temp_dir.path();
        let cert_path = test_dir_path.join("test_cert.pem");
        fs::write(&cert_path, "").unwrap();
        let yaml = YamlLoader::load_from_str(&format!("|-\n  {}", cert_path.display())).unwrap();
        assert!(as_rustls_certificates(&yaml[0], Some(temp_dir.path())).is_err());

        // non-string element in array
        let yaml = Yaml::Array(vec![Yaml::Integer(123)]);
        assert!(as_rustls_certificates(&yaml, None).is_err());
    }

    #[test]
    fn as_rustls_private_key_ok() {
        let temp_dir = TempDir::new("rustls_key_ok");
        let test_dir_path = temp_dir.path();
        let key_path = test_dir_path.join("test_key.pem");
        fs::write(&key_path, TEST_KEY_PEM).unwrap();

        // single PEM string
        let yaml = Yaml::String(TEST_KEY_PEM.to_string());
        let key = as_rustls_private_key(&yaml, None).unwrap();
        assert!(matches!(key, PrivateKeyDer::Pkcs8(_)));

        // file path
        let yaml = YamlLoader::load_from_str(&format!("|-\n  {}", key_path.display())).unwrap();
        let key = as_rustls_private_key(&yaml[0], Some(temp_dir.path())).unwrap();
        assert!(matches!(key, PrivateKeyDer::Pkcs8(_)));
    }

    #[test]
    fn as_rustls_private_key_err() {
        // invalid PEM string
        let yaml = yaml_str!("invalid key");
        assert!(as_rustls_private_key(&yaml, None).is_err());

        // non-existent file
        let yaml = yaml_str!("non_existent_key.pem");
        assert!(as_rustls_private_key(&yaml, Some(Path::new("/non_existent"))).is_err());

        // empty string
        let yaml = yaml_str!("");
        assert!(as_rustls_private_key(&yaml, None).is_err());

        // null value
        let yaml = Yaml::Null;
        assert!(as_rustls_private_key(&yaml, None).is_err());
    }

    #[test]
    fn as_rustls_certificate_pair_ok() {
        let temp_dir = TempDir::new("rustls_cert_pair_ok");
        let test_dir_path = temp_dir.path();
        let cert_path = test_dir_path.join("test_cert.pem");
        fs::write(&cert_path, TEST_CERT_PEM).unwrap();
        let key_path = test_dir_path.join("test_key.pem");
        fs::write(&key_path, TEST_KEY_PEM).unwrap();

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
        let pair = as_rustls_certificate_pair(&yaml[0], None).unwrap();
        let mut expected = RustlsCertificatePairBuilder::default();
        expected.set_certs(vec![
            CertificateDer::from_pem_slice(TEST_CERT_PEM.as_bytes()).unwrap(),
        ]);
        expected.set_key(PrivateKeyDer::from_pem_slice(TEST_KEY_PEM.as_bytes()).unwrap());
        assert_eq!(pair, expected.build().unwrap());
    }

    #[test]
    fn as_rustls_certificate_pair_err() {
        // non-map YAML
        let yaml = Yaml::Integer(123);
        assert!(as_rustls_certificate_pair(&yaml, None).is_err());

        let yaml = yaml_str!("invalid");
        assert!(as_rustls_certificate_pair(&yaml, None).is_err());

        // empty map
        let yaml = yaml_doc!(r#"{}"#);
        assert!(as_rustls_certificate_pair(&yaml, None).is_err());

        // invalid certificate
        let yaml = yaml_doc!(
            r#"
                certificate: "invalid cert"
            "#
        );
        assert!(as_rustls_certificate_pair(&yaml[0], None).is_err());

        // invalid private key
        let yaml = yaml_doc!(
            r#"
                private_key: "invalid key"
            "#
        );
        assert!(as_rustls_certificate_pair(&yaml[0], None).is_err());

        // unknown key
        let yaml = yaml_doc!(
            r#"
                unknown_key: "value"
            "#
        );
        assert!(as_rustls_certificate_pair(&yaml[0], None).is_err());
    }

    #[test]
    fn as_rustls_client_config_builder_ok() {
        let temp_dir = TempDir::new("rustls_client_config_builder_ok");
        let test_dir_path = temp_dir.path();
        let cert_path = test_dir_path.join("test_cert.pem");
        fs::write(&cert_path, TEST_CERT_PEM).unwrap();
        let key_path = test_dir_path.join("test_key.pem");
        fs::write(&key_path, TEST_KEY_PEM).unwrap();
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
        let cert_pair1 = as_rustls_certificate_pair(&yaml[0], Some(test_dir_path)).unwrap();
        let cert_pair2 = as_rustls_certificate_pair(&yaml[0], Some(test_dir_path)).unwrap();

        let yaml = YamlLoader::load_from_str(&format!(
            r#"
                no_session_cache: true
                disable_sni: true
                max_fragment_size: 1400
                certificate: |-
                    {}
                private_key: |-
                    {}
                ca_certificate: |-
                    {}
                no_default_ca_certificate: true
                use_builtin_ca_certificate: true
                handshake_timeout: "10s"
            "#,
            cert_path.display(),
            key_path.display(),
            cert_path.display()
        ))
        .unwrap();
        let builder = as_rustls_client_config_builder(&yaml[0], None).unwrap();
        let mut expected = RustlsClientConfigBuilder::default();
        expected.set_no_session_cache();
        expected.set_disable_sni();
        expected.set_max_fragment_size(1400);
        expected.set_cert_pair(cert_pair1);
        let ca_cert = CertificateDer::from_pem_slice(TEST_CERT_PEM.as_bytes()).unwrap();
        expected.set_ca_certificates(vec![ca_cert]);
        expected.set_no_default_ca_certificates();
        expected.set_use_builtin_ca_certificates();
        expected.set_negotiation_timeout(Duration::from_secs(10));
        assert_eq!(builder, expected);

        // cert_pair field
        let yaml = YamlLoader::load_from_str(&format!(
            r#"
                cert_pair:
                    certificate: |-
                        {}
                    private_key: |-
                        {}
            "#,
            cert_path.display(),
            key_path.display()
        ))
        .unwrap();
        let builder = as_rustls_client_config_builder(&yaml[0], None).unwrap();
        let mut expected = RustlsClientConfigBuilder::default();
        expected.set_cert_pair(cert_pair2);
        assert_eq!(builder, expected);
    }

    #[test]
    fn as_rustls_client_config_builder_err() {
        // non-map YAML
        let yaml = Yaml::Integer(123);
        assert!(as_rustls_client_config_builder(&yaml, None).is_err());

        let yaml = yaml_str!("invalid");
        assert!(as_rustls_client_config_builder(&yaml, None).is_err());

        // invalid boolean value
        let yaml = yaml_doc!(r#"no_session_cache: "not_a_bool""#);
        assert!(as_rustls_client_config_builder(&yaml[0], None).is_err());

        // invalid max_fragment_size
        let yaml = yaml_doc!(r#"max_fragment_size: "invalid""#);
        assert!(as_rustls_client_config_builder(&yaml[0], None).is_err());

        // duplicate certificate config
        let temp_dir = TempDir::new("rustls_client_config_builder_err");
        let test_dir_path = temp_dir.path();
        let cert_path = test_dir_path.join("test_cert.pem");
        fs::write(&cert_path, TEST_CERT_PEM).unwrap();
        let key_path = test_dir_path.join("test_key.pem");
        fs::write(&key_path, TEST_KEY_PEM).unwrap();
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
        assert!(as_rustls_client_config_builder(&yaml[0], None).is_err());

        // unknown key
        let yaml = yaml_doc!(
            r#"
                unknown_key: "value"
            "#
        );
        assert!(as_rustls_client_config_builder(&yaml[0], None).is_err());
    }

    #[test]
    fn as_rustls_server_config_builder_ok() {
        let temp_dir = TempDir::new("rustls_server_config_builder_ok");
        let test_dir_path = temp_dir.path();
        let cert_path = test_dir_path.join("test_cert.pem");
        fs::write(&cert_path, TEST_CERT_PEM).unwrap();
        let key_path = test_dir_path.join("test_key.pem");
        fs::write(&key_path, TEST_KEY_PEM).unwrap();
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
        let cert_pair1 = as_rustls_certificate_pair(&yaml[0], Some(test_dir_path)).unwrap();
        let cert_pair2 = as_rustls_certificate_pair(&yaml[0], Some(test_dir_path)).unwrap();
        let cert_pair3 = as_rustls_certificate_pair(&yaml[0], Some(test_dir_path)).unwrap();

        let yaml = YamlLoader::load_from_str(&format!(
            r#"
                cert_pairs:
                  - certificate: |-
                        {}
                    private_key: |-
                        {}
                enable_client_auth: true
                use_session_ticket: false
                no_session_cache: true
                ca_certificate: |-
                    {}
                handshake_timeout: "10s"
            "#,
            cert_path.display(),
            key_path.display(),
            cert_path.display()
        ))
        .unwrap();
        let builder = as_rustls_server_config_builder(&yaml[0], None).unwrap();
        let mut expected = RustlsServerConfigBuilder::empty();
        expected.push_cert_pair(cert_pair1);
        expected.enable_client_auth();
        expected.set_use_session_ticket(false);
        expected.set_disable_session_cache(true);
        let ca_cert = CertificateDer::from_pem_slice(TEST_CERT_PEM.as_bytes()).unwrap();
        expected.set_client_auth_certificates(vec![ca_cert]);
        expected.set_accept_timeout(Duration::from_secs(10));
        assert_eq!(builder, expected);

        // cert_pair without array
        let yaml = YamlLoader::load_from_str(&format!(
            r#"
                cert_pairs:
                    certificate: |-
                        {}
                    private_key: |-
                        {}
                no_session_ticket: true
            "#,
            cert_path.display(),
            key_path.display()
        ))
        .unwrap();
        let builder = as_rustls_server_config_builder(&yaml[0], None).unwrap();
        let mut expected = RustlsServerConfigBuilder::empty();
        expected.push_cert_pair(cert_pair2);
        expected.set_disable_session_ticket(true);
        assert_eq!(builder, expected);

        // certificate and private_key fields
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
        let builder = as_rustls_server_config_builder(&yaml[0], None).unwrap();
        let mut expected = RustlsServerConfigBuilder::empty();
        expected.push_cert_pair(cert_pair3);
        assert_eq!(builder, expected);
    }

    #[test]
    fn as_rustls_server_config_builder_err() {
        // non-map YAML
        let yaml = Yaml::Integer(123);
        assert!(as_rustls_server_config_builder(&yaml, None).is_err());

        let yaml = yaml_str!("invalid");
        assert!(as_rustls_server_config_builder(&yaml, None).is_err());

        // empty config (no cert_pairs)
        let yaml = yaml_doc!(r#"enable_client_auth: true"#);
        assert!(as_rustls_server_config_builder(&yaml, None).is_err());

        // invalid cert_pairs array element
        let yaml = yaml_doc!(
            r#"
                cert_pairs:
                  - invalid
            "#
        );
        assert!(as_rustls_server_config_builder(&yaml[0], None).is_err());

        // invalid boolean value
        let yaml = yaml_doc!(
            r#"
                no_session_cache: "not_a_bool"
            "#
        );
        assert!(as_rustls_server_config_builder(&yaml[0], None).is_err());

        // unknown key
        let yaml = yaml_doc!(
            r#"
                unknown_key: "value"
            "#
        );
        assert!(as_rustls_server_config_builder(&yaml[0], None).is_err());
    }
}
