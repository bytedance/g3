/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_dpi::{ProtocolInspectionConfig, ProtocolInspectionSizeLimit};

pub fn parse_inspect_size_limit(
    config: &mut ProtocolInspectionSizeLimit,
    value: &Yaml,
) -> anyhow::Result<()> {
    if let Yaml::Hash(map) = value {
        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "ftp_greeting_msg" | "ftp_server_greeting_msg" => {
                let size = crate::humanize::as_usize(v)
                    .context(format!("invalid humanize usize value for key {k}"))?;
                config.set_ftp_server_greeting_msg(size);
                Ok(())
            }
            "http_request_uri" | "http_client_request_uri" => {
                let size = crate::humanize::as_usize(v)
                    .context(format!("invalid humanize usize value for key {k}"))?;
                config.set_http_client_request_uri(size);
                Ok(())
            }
            "imap_greeting_msg" | "imap_server_greeting_msg" => {
                let size = crate::humanize::as_usize(v)
                    .context(format!("invalid humanize usize value for key {k}"))?;
                config.set_imap_server_greeting_msg(size);
                Ok(())
            }
            "nats_info_line" | "nats_server_info_line" => {
                let size = crate::humanize::as_usize(v)
                    .context(format!("invalid humanize usize value for key {k}"))?;
                config.set_nats_server_info_line(size);
                Ok(())
            }
            "smtp_greeting_msg" | "smtp_server_greeting_msg" => Ok(()),
            _ => Err(anyhow!("invalid key {k}")),
        })
    } else {
        Err(anyhow!(
            "yaml value type for 'inspect size limit' should be 'map'"
        ))
    }
}

pub fn as_protocol_inspection_config(value: &Yaml) -> anyhow::Result<ProtocolInspectionConfig> {
    if let Yaml::Hash(map) = value {
        let mut config = ProtocolInspectionConfig::default();

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "data0_buffer_size" => {
                let size = crate::humanize::as_usize(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                config.set_data0_buffer_size(size);
                Ok(())
            }
            "inspect_max_depth" => {
                let depth = crate::value::as_usize(v)?;
                config.set_max_depth(depth);
                Ok(())
            }
            "data0_wait_timeout" => {
                let value = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                config.set_data0_wait_timeout(value);
                Ok(())
            }
            "data0_read_timeout" => {
                let value = crate::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                config.set_data0_read_timeout(value);
                Ok(())
            }
            "data0_size_limit" => parse_inspect_size_limit(config.size_limit_mut(), v)
                .context(format!("invalid inspect size limit value for key {k}")),
            _ => Err(anyhow!("invalid key {k}")),
        })?;

        Ok(config)
    } else {
        Err(anyhow!(
            "yaml value type for 'protocol inspection config' should be 'map'"
        ))
    }
}

#[cfg(test)]
#[cfg(feature = "dpi")]
mod test {
    use super::*;
    use std::time::Duration;
    use yaml_rust::YamlLoader;

    #[test]
    fn parse_inspect_size_limit_ok() {
        // full valid key and value
        let yaml = yaml_doc!(
            r"
                ftp_greeting_msg: 1KB
                http_request_uri: 2MB
                imap_greeting_msg: 512B
                nats_info_line: 1KB
                smtp_greeting_msg: 1KB
            "
        );
        let mut limit = ProtocolInspectionSizeLimit::default();
        parse_inspect_size_limit(&mut limit, &yaml).expect("valid config should parse");
        let mut expected = ProtocolInspectionSizeLimit::default();
        expected.set_ftp_server_greeting_msg(1000);
        expected.set_http_client_request_uri(2 * 1000 * 1000);
        expected.set_imap_server_greeting_msg(512);
        expected.set_nats_server_info_line(1000);
        assert_eq!(limit, expected);

        // alias key
        let yaml = yaml_doc!(
            r"
                ftp_server_greeting_msg: 2KB
                http_client_request_uri: 4MB
                imap_server_greeting_msg: 1KB
                nats_server_info_line: 2KB
                smtp_server_greeting_msg: 2KB
            "
        );
        let mut limit = ProtocolInspectionSizeLimit::default();
        parse_inspect_size_limit(&mut limit, &yaml).expect("valid aliases should parse");
        let mut expected = ProtocolInspectionSizeLimit::default();
        expected.set_ftp_server_greeting_msg(2 * 1000);
        expected.set_http_client_request_uri(4 * 1000 * 1000);
        expected.set_imap_server_greeting_msg(1000);
        expected.set_nats_server_info_line(2 * 1000);
        assert_eq!(limit, expected);

        // default value
        let yaml = Yaml::Hash(Default::default());
        let mut limit = ProtocolInspectionSizeLimit::default();
        parse_inspect_size_limit(&mut limit, &yaml).expect("default value should parse");
        let expected = ProtocolInspectionSizeLimit::default();
        assert_eq!(limit, expected);
    }

    #[test]
    fn parse_inspect_size_limit_err() {
        let mut limit = ProtocolInspectionSizeLimit::default();

        // invalid value for key
        let yaml = yaml_doc!("ftp_greeting_msg: invalid");
        assert!(parse_inspect_size_limit(&mut limit, &yaml).is_err());

        let yaml = yaml_doc!("http_request_uri: -1KB");
        assert!(parse_inspect_size_limit(&mut limit, &yaml).is_err());

        let yaml = yaml_doc!("imap_greeting_msg: 2MBX");
        assert!(parse_inspect_size_limit(&mut limit, &yaml).is_err());

        let yaml = yaml_doc!("nats_info_line: 512BY");
        assert!(parse_inspect_size_limit(&mut limit, &yaml).is_err());

        // invalid key
        let yaml = yaml_doc!("invalid_key: value");
        assert!(parse_inspect_size_limit(&mut limit, &yaml).is_err());

        // non-map input
        let yaml = yaml_str!("invalid");
        assert!(parse_inspect_size_limit(&mut limit, &yaml).is_err());

        let yaml = Yaml::Null;
        assert!(parse_inspect_size_limit(&mut limit, &yaml).is_err());
    }

    #[test]
    fn as_protocol_inspection_config_ok() {
        // full valid configuration
        let yaml = yaml_doc!(
            r"
                data0_buffer_size: 8KB
                inspect_max_depth: 5
                data0_wait_timeout: 30s
                data0_read_timeout: 2s
                data0_size_limit:
                    ftp_server_greeting_msg: 1KB
                    http_client_request_uri: 2MB
                    imap_server_greeting_msg: 512B
                    nats_server_info_line: 1KB
                    smtp_server_greeting_msg: 1KB
            "
        );
        let config = as_protocol_inspection_config(&yaml).expect("valid config should parse");
        let mut expected_config = ProtocolInspectionConfig::default();
        expected_config.set_data0_buffer_size(8000);
        expected_config.set_max_depth(5);
        expected_config.set_data0_wait_timeout(Duration::from_secs(30));
        expected_config.set_data0_read_timeout(Duration::from_secs(2));
        let mut size_limit = ProtocolInspectionSizeLimit::default();
        size_limit.set_ftp_server_greeting_msg(1000);
        size_limit.set_http_client_request_uri(2 * 1000 * 1000);
        size_limit.set_imap_server_greeting_msg(512);
        size_limit.set_nats_server_info_line(1000);
        *expected_config.size_limit_mut() = size_limit;
        assert_eq!(config, expected_config);

        // default configuration
        let yaml = Yaml::Hash(Default::default());
        let config = as_protocol_inspection_config(&yaml).expect("default value should parse");
        let expected_config = ProtocolInspectionConfig::default();
        assert_eq!(config, expected_config);
    }

    #[test]
    fn as_protocol_inspection_config_err() {
        // invalid value for key
        let yaml = yaml_doc!("data0_buffer_size: invalid");
        assert!(as_protocol_inspection_config(&yaml).is_err());

        let yaml = yaml_doc!("inspect_max_depth: -1");
        assert!(as_protocol_inspection_config(&yaml).is_err());

        let yaml = yaml_doc!("data0_wait_timeout: invalid");
        assert!(as_protocol_inspection_config(&yaml).is_err());

        let yaml = yaml_doc!("data0_read_timeout: -1s");
        assert!(as_protocol_inspection_config(&yaml).is_err());

        let yaml = yaml_doc!("data0_size_limit: ");
        assert!(as_protocol_inspection_config(&yaml).is_err());

        // invalid key
        let yaml = yaml_doc!("invalid_key: value");
        assert!(as_protocol_inspection_config(&yaml).is_err());

        // non-map input
        let yaml = yaml_str!("invalid");
        assert!(as_protocol_inspection_config(&yaml).is_err());

        let yaml = Yaml::Integer(123);
        assert!(as_protocol_inspection_config(&yaml).is_err());
    }
}
