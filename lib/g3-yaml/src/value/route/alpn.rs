/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_types::metrics::NodeName;
use g3_types::route::AlpnMatch;

use crate::{YamlDocPosition, YamlMapCallback};

fn add_alpn_matched_value<T: YamlMapCallback>(
    obj: &mut AlpnMatch<Arc<T>>,
    value: &Yaml,
    mut target: T,
    doc: Option<&YamlDocPosition>,
) -> anyhow::Result<()> {
    let type_name = target.type_name();

    if let Yaml::Hash(map) = value {
        let mut protocol_vs = vec![];
        let mut set_default = false;

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "set_default" => {
                set_default =
                    crate::value::as_bool(v).context(format!("invalid bool value for key {k}"))?;
                Ok(())
            }
            "protocol" => {
                if let Yaml::Array(seq) = v {
                    for (i, v) in seq.iter().enumerate() {
                        let protocol = crate::value::as_string(v)
                            .context(format!("invalid string value for {k}#{i}"))?;
                        protocol_vs.push(protocol);
                    }
                } else {
                    let protocol = crate::value::as_string(v)
                        .context(format!("invalid string value for {k}"))?;
                    protocol_vs.push(protocol);
                }
                Ok(())
            }
            normalized_key => target
                .parse_kv(normalized_key, v, doc)
                .context(format!("failed to parse {type_name} value for key {k}")),
        })?;

        target
            .check()
            .context(format!("{type_name} final check failed"))?;

        let t = Arc::new(target);
        let mut auto_default = true;
        for protocol in protocol_vs {
            if obj.add_protocol(protocol.clone(), Arc::clone(&t)).is_some() {
                return Err(anyhow!(
                    "duplicate {type_name} value for protocol {protocol}"
                ));
            }
            auto_default = false;
        }
        if (set_default || auto_default) && obj.set_default(t).is_some() {
            return Err(anyhow!("a default {type_name} value has already been set"));
        }

        Ok(())
    } else {
        Err(anyhow!(
            "yaml type for 'alpn matched {type_name} value' should be 'map'"
        ))
    }
}

pub fn as_alpn_matched_obj<T>(
    value: &Yaml,
    doc: Option<&YamlDocPosition>,
) -> anyhow::Result<AlpnMatch<Arc<T>>>
where
    T: Default + YamlMapCallback,
{
    let mut obj = AlpnMatch::<Arc<T>>::default();

    if let Yaml::Array(seq) = value {
        for (i, v) in seq.iter().enumerate() {
            let target = T::default();
            let type_name = target.type_name();
            add_alpn_matched_value(&mut obj, v, target, doc).context(format!(
                "invalid alpn matched {type_name} value for element #{i}"
            ))?;
        }
    } else {
        let target = T::default();
        let type_name = target.type_name();
        add_alpn_matched_value(&mut obj, value, target, doc)
            .context(format!("invalid alpn matched {type_name} value"))?;
    }

    Ok(obj)
}

fn add_alpn_matched_backend(obj: &mut AlpnMatch<NodeName>, value: &Yaml) -> anyhow::Result<()> {
    let mut protocol_vs = vec![];
    let mut set_default = false;
    let mut name = NodeName::default();

    if let Yaml::Hash(map) = value {
        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "set_default" => {
                set_default =
                    crate::value::as_bool(v).context(format!("invalid bool value for key {k}"))?;
                Ok(())
            }
            "protocol" => {
                if let Yaml::Array(seq) = v {
                    for (i, v) in seq.iter().enumerate() {
                        let protocol = crate::value::as_string(v)
                            .context(format!("invalid string value for {k}#{i}"))?;
                        protocol_vs.push(protocol);
                    }
                } else {
                    let protocol = crate::value::as_string(v)
                        .context(format!("invalid string value for {k}"))?;
                    protocol_vs.push(protocol);
                }
                Ok(())
            }
            "backend" => {
                name = crate::value::as_metric_node_name(v)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;
    } else {
        name = crate::value::as_metric_node_name(value)?;
    }

    let mut auto_default = true;
    for protocol in protocol_vs {
        if obj.add_protocol(protocol.clone(), name.clone()).is_some() {
            return Err(anyhow!("duplicate value for protocol {protocol}"));
        }
        auto_default = false;
    }
    if (set_default || auto_default) && obj.set_default(name).is_some() {
        return Err(anyhow!("a default value has already been set"));
    }
    Ok(())
}

pub fn as_alpn_matched_backends(value: &Yaml) -> anyhow::Result<AlpnMatch<NodeName>> {
    let mut obj = AlpnMatch::<NodeName>::default();

    if let Yaml::Array(seq) = value {
        for (i, v) in seq.iter().enumerate() {
            add_alpn_matched_backend(&mut obj, v)
                .context(format!("invalid alpn matched name value for element #{i}"))?;
        }
    } else {
        add_alpn_matched_backend(&mut obj, value).context("invalid alpn matched name value")?;
    }

    Ok(obj)
}

#[cfg(test)]
#[cfg(feature = "route")]
mod tests {
    use super::*;
    use yaml_rust::YamlLoader;

    // Mock configuration for testing as_alpn_matched_obj
    #[derive(Default, Debug, PartialEq)]
    struct TestConfig {
        field: String,
        count: u32,
    }

    impl YamlMapCallback for TestConfig {
        fn type_name(&self) -> &'static str {
            "TestConfig"
        }

        fn parse_kv(
            &mut self,
            key: &str,
            value: &Yaml,
            _doc: Option<&YamlDocPosition>,
        ) -> anyhow::Result<()> {
            match key {
                "field" => {
                    self.field =
                        crate::value::as_string(value).context("invalid string for field")?;
                    Ok(())
                }
                "count" => {
                    self.count = crate::value::as_u32(value).context("invalid u32 for count")?;
                    Ok(())
                }
                _ => Err(anyhow!("invalid key: {key}")),
            }
        }

        fn check(&mut self) -> anyhow::Result<()> {
            if self.field.is_empty() {
                return Err(anyhow!("field cannot be empty"));
            }
            Ok(())
        }
    }

    #[test]
    fn as_alpn_matched_obj_ok() {
        // Single element configuration
        let yaml = yaml_doc!(
            r#"
                protocol: h2
                field: test_value
            "#
        );
        let result = as_alpn_matched_obj::<TestConfig>(&yaml, None).unwrap();
        assert_eq!(result.get("h2").unwrap().field, "test_value");
        assert_eq!(result.get_default(), None);

        // Multiple protocols in array
        let yaml = yaml_doc!(
            r#"
                - protocol: [h2, http/1.1]
                  field: multi_protocol
                - protocol: h3
                  set_default: true
                  field: default_protocol
            "#
        );
        let result = as_alpn_matched_obj::<TestConfig>(&yaml, None).unwrap();
        assert_eq!(result.get("h2").unwrap().field, "multi_protocol");
        assert_eq!(result.get("http/1.1").unwrap().field, "multi_protocol");
        assert_eq!(result.get("h3").unwrap().field, "default_protocol");
        assert_eq!(result.get_default().unwrap().field, "default_protocol");

        // Auto default when no protocol specified
        let yaml = yaml_doc!(
            r#"
                field: auto_default_test
            "#
        );
        let result = as_alpn_matched_obj::<TestConfig>(&yaml, None).unwrap();
        assert_eq!(result.get_default().unwrap().field, "auto_default_test");
    }

    #[test]
    fn as_alpn_matched_obj_err() {
        // Invalid YAML type
        let yaml = yaml_str!("invalid");
        assert!(as_alpn_matched_obj::<TestConfig>(&yaml, None).is_err());

        // Duplicate protocol
        let yaml = yaml_doc!(
            r#"
                - protocol: h2
                  field: first
                - protocol: h2
                  field: second
            "#
        );
        assert!(as_alpn_matched_obj::<TestConfig>(&yaml, None).is_err());

        // Duplicate default value
        let yaml = yaml_doc!(
            r#"
                - set_default: true
                  field: first
                - set_default: true
                  field: second
            "#
        );
        assert!(as_alpn_matched_obj::<TestConfig>(&yaml, None).is_err());

        // Config check failure
        let yaml = yaml_doc!(
            r#"
                count: 5  # field is required but missing
            "#
        );
        assert!(as_alpn_matched_obj::<TestConfig>(&yaml, None).is_err());

        // Invalid key in hash
        let yaml = yaml_doc!(
            r#"
                invalid_key: value
                field: test_value
            "#
        );
        assert!(as_alpn_matched_obj::<TestConfig>(&yaml, None).is_err());

        // Missing value for field
        let yaml = yaml_doc!(
            r#"
                field: 
            "#
        );
        assert!(as_alpn_matched_obj::<TestConfig>(&yaml, None).is_err());
    }

    #[test]
    fn as_alpn_matched_backends_ok() {
        // Simple backend name
        let yaml = yaml_str!("backend_name");
        let result = as_alpn_matched_backends(&yaml).unwrap();
        assert_eq!(result.get_default().unwrap().as_str(), "backend_name");

        // Hash configuration with single protocol
        let yaml = yaml_doc!(
            r#"
                protocol: h2
                backend: specific_backend
            "#
        );
        let result = as_alpn_matched_backends(&yaml).unwrap();
        assert_eq!(result.get("h2").unwrap().as_str(), "specific_backend");
        assert_eq!(result.get_default(), None);

        // Array configuration with multiple entries
        let yaml = yaml_doc!(
            r#"
                - protocol: [h2, http/1.1]
                  backend: multi_protocol_backend
                - backend: default_backend
                  set_default: true
            "#
        );
        let result = as_alpn_matched_backends(&yaml).unwrap();
        assert_eq!(result.get("h2").unwrap().as_str(), "multi_protocol_backend");
        assert_eq!(
            result.get("http/1.1").unwrap().as_str(),
            "multi_protocol_backend"
        );
        assert_eq!(result.get_default().unwrap().as_str(), "default_backend");

        // Auto default when no protocol specified
        let yaml = yaml_doc!(
            r#"
                backend: auto_default_backend
            "#
        );
        let result = as_alpn_matched_backends(&yaml).unwrap();
        assert_eq!(
            result.get_default().unwrap().as_str(),
            "auto_default_backend"
        );
    }

    #[test]
    fn as_alpn_matched_backends_err() {
        // Duplicate protocol
        let yaml = yaml_doc!(
            r#"
                - protocol: h2
                  backend: first
                - protocol: h2
                  backend: second
            "#
        );
        assert!(as_alpn_matched_backends(&yaml).is_err());

        // Duplicate default value
        let yaml = yaml_doc!(
            r#"
                - set_default: true
                  backend: first
                - set_default: true
                  backend: second
            "#
        );
        assert!(as_alpn_matched_backends(&yaml).is_err());

        // Invalid key in hash
        let yaml = yaml_doc!(
            r#"
                invalid_key: value
                backend: test
            "#
        );
        assert!(as_alpn_matched_backends(&yaml).is_err());

        // Invalid value type for backend name
        let yaml = yaml_doc!(
            r#"
                backend: 123
            "#
        );
        assert!(as_alpn_matched_backends(&yaml).is_err());
    }
}
