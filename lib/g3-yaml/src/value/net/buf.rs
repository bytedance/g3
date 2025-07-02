/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_types::net::SocketBufferConfig;

pub fn as_socket_buffer_config(value: &Yaml) -> anyhow::Result<SocketBufferConfig> {
    let mut config = SocketBufferConfig::default();

    match value {
        Yaml::Integer(_) | Yaml::String(_) => {
            let size =
                crate::humanize::as_usize(value).context("invalid single humanize usize value")?;
            config.set_recv_size(size);
            config.set_send_size(size);
        }
        Yaml::Hash(map) => {
            crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
                "recv" | "receive" | "read" => {
                    let size = crate::humanize::as_usize(v)
                        .context(format!("invalid humanize usize value for key {k}"))?;
                    config.set_recv_size(size);
                    Ok(())
                }
                "send" | "write" => {
                    let size = crate::humanize::as_usize(v)
                        .context(format!("invalid humanize usize value for key {k}"))?;
                    config.set_send_size(size);
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;
        }
        _ => return Err(anyhow!("invalid yaml value: {:?}", value)),
    }

    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    use yaml_rust::YamlLoader;

    #[test]
    fn test_single_integer() {
        let yaml = Yaml::Integer(1024);
        let config = as_socket_buffer_config(&yaml).unwrap();
        assert_eq!(config.recv_size(), Some(1024));
        assert_eq!(config.send_size(), Some(1024));
    }

    #[test]
    fn test_single_string() {
        let yaml = Yaml::String("2M".to_string());
        let config = as_socket_buffer_config(&yaml).unwrap();
        assert_eq!(config.recv_size(), Some(2_000_000));
        assert_eq!(config.send_size(), Some(2_000_000));
    }

    #[test]
    fn test_invalid_single_humanize_usize_value() {
        let yaml = Yaml::String("invalid".to_string());
        assert!(as_socket_buffer_config(&yaml).is_err());
    }

    #[test]
    fn test_hash_standard_keys() {
        let s = "
        recv: 512
        send: 1024
        ";
        let docs = YamlLoader::load_from_str(s).unwrap();
        let doc = &docs[0];

        let config = as_socket_buffer_config(doc).unwrap();
        assert_eq!(config.recv_size(), Some(512));
        assert_eq!(config.send_size(), Some(1024));
    }

    #[test]
    fn test_hash_variant_keys() {
        let s = "
        receive: 256
        write: 512
        ";
        let docs = YamlLoader::load_from_str(s).unwrap();
        let doc = &docs[0];

        let config = as_socket_buffer_config(doc).unwrap();
        assert_eq!(config.recv_size(), Some(256));
        assert_eq!(config.send_size(), Some(512));
    }

    #[test]
    fn test_invalid_key() {
        let s = "
        invalid_key: 100
        ";
        let docs = YamlLoader::load_from_str(s).unwrap();
        let doc = &docs[0];

        let result = as_socket_buffer_config(doc);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_type() {
        let yaml = Yaml::Boolean(true);
        let result = as_socket_buffer_config(&yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_partial_config() {
        let s = "read: 2048";
        let docs = YamlLoader::load_from_str(s).unwrap();
        let doc = &docs[0];

        let config = as_socket_buffer_config(doc).unwrap();
        assert_eq!(config.recv_size(), Some(2048));
        assert_eq!(config.send_size(), None);
    }

    #[test]
    fn test_empty_string() {
        let yaml = Yaml::String("".to_string());
        let result = as_socket_buffer_config(&yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_invalid_humanize_value() {
        let cases = vec![
            ("read: invalid", "read"),
            ("write: invalid", "write"),
            ("recv: 10XYZ", "recv"),
        ];

        for (yaml_str, _key) in cases {
            let docs = YamlLoader::load_from_str(yaml_str).unwrap();
            let doc = &docs[0];

            let result = as_socket_buffer_config(doc);
            assert!(result.is_err(), "Case failed: {}", yaml_str);
        }
    }
}
