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
