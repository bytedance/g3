/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_types::route::UriPathMatch;

use crate::{YamlDocPosition, YamlMapCallback};

fn add_url_path_matched_value<T: YamlMapCallback>(
    obj: &mut UriPathMatch<Arc<T>>,
    value: &Yaml,
    mut target: T,
    doc: Option<&YamlDocPosition>,
) -> anyhow::Result<()> {
    let type_name = target.type_name();

    if let Yaml::Hash(map) = value {
        let mut prefix_match_vs = vec![];
        let mut set_default = false;

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "set_default" => {
                set_default =
                    crate::value::as_bool(v).context(format!("invalid bool value for key {k}"))?;
                Ok(())
            }
            "prefix_match" => {
                let prefix = crate::value::as_string(v)
                    .context(format!("invalid string value for key {k}"))?;
                prefix_match_vs.push(prefix);
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
        for prefix in &prefix_match_vs {
            if obj.add_prefix(prefix.to_string(), Arc::clone(&t)).is_some() {
                return Err(anyhow!(
                    "duplicate {type_name} value for path prefix {prefix}"
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
            "yaml type for 'url path matched {type_name} value' should be 'map'"
        ))
    }
}

pub fn as_url_path_matched_obj<T>(
    value: &Yaml,
    doc: Option<&YamlDocPosition>,
) -> anyhow::Result<UriPathMatch<Arc<T>>>
where
    T: Default + YamlMapCallback,
{
    let mut obj = UriPathMatch::<Arc<T>>::default();

    if let Yaml::Array(seq) = value {
        for (i, v) in seq.iter().enumerate() {
            let target = T::default();
            let type_name = target.type_name();
            add_url_path_matched_value(&mut obj, v, target, doc).context(format!(
                "invalid url path matched {type_name} value for element #{i}"
            ))?;
        }
    } else {
        let target = T::default();
        let type_name = target.type_name();
        add_url_path_matched_value(&mut obj, value, target, doc)
            .context(format!("invalid url path matched {type_name} value"))?;
    }

    Ok(obj)
}

#[cfg(test)]
#[cfg(feature = "route")]
mod tests {
    use super::*;
    use yaml_rust::YamlLoader;

    // Test structure implementing YamlMapCallback
    #[derive(Default, PartialEq, Debug)]
    struct TestCallback {
        id: u32,
        enabled: bool,
    }

    impl YamlMapCallback for TestCallback {
        fn type_name(&self) -> &'static str {
            "TestUriPathCallback"
        }

        fn parse_kv(
            &mut self,
            key: &str,
            value: &Yaml,
            _doc: Option<&YamlDocPosition>,
        ) -> anyhow::Result<()> {
            match key {
                "id" => {
                    self.id = value.as_i64().ok_or(anyhow!("invalid integer"))? as u32;
                    Ok(())
                }
                "enabled" => {
                    self.enabled = crate::value::as_bool(value)?;
                    Ok(())
                }
                _ => Err(anyhow!("unknown key: {}", key)),
            }
        }

        fn check(&mut self) -> anyhow::Result<()> {
            if self.id == 0 {
                Err(anyhow!("id cannot be zero"))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn as_url_path_matched_obj_ok() {
        // Single object with prefix_match
        let yaml = yaml_doc!(
            r#"
                prefix_match: /api
                id: 1
                enabled: true
            "#
        );
        let path_match = as_url_path_matched_obj::<TestCallback>(&yaml, None).unwrap();
        let value = path_match.get("/api").unwrap();
        assert_eq!(value.id, 1);
        assert!(value.enabled);

        // Single object with set_default
        let yaml = yaml_doc!(
            r#"
                set_default: true
                id: 2
                enabled: false
            "#
        );
        let path_match = as_url_path_matched_obj::<TestCallback>(&yaml, None).unwrap();
        let default_value = path_match.get("/unmatched").unwrap();
        assert_eq!(default_value.id, 2);
        assert!(!default_value.enabled);

        // Auto default
        let yaml = yaml_doc!(
            r#"
                id: 3
                enabled: true
            "#
        );
        let path_match = as_url_path_matched_obj::<TestCallback>(&yaml, None).unwrap();
        let value = path_match.get("/any/path").unwrap();
        assert_eq!(value.id, 3);
        assert!(value.enabled);

        // Array input with multiple objects
        let yaml = yaml_doc!(
            r#"
                - prefix_match: /v1
                  id: 10
                - prefix_match: /v2
                  id: 20
                - set_default: true
                  id: 30
            "#
        );
        let path_match = as_url_path_matched_obj::<TestCallback>(&yaml, None).unwrap();
        assert_eq!(path_match.get("/v1").unwrap().id, 10);
        assert_eq!(path_match.get("/v2").unwrap().id, 20);
        assert_eq!(path_match.get("/unknown").unwrap().id, 30);

        // Multiple prefix matches
        let yaml = yaml_doc!(
            r#"
                - prefix_match: /static
                  id: 100
                - prefix_match: /images
                  id: 100
            "#
        );
        let path_match = as_url_path_matched_obj::<TestCallback>(&yaml, None).unwrap();
        assert_eq!(path_match.get("/static").unwrap().id, 100);
        assert_eq!(path_match.get("/images").unwrap().id, 100);
    }

    #[test]
    fn as_url_path_matched_obj_err() {
        // Duplicate prefix
        let yaml = yaml_doc!(
            r#"
                - prefix_match: /api
                  id: 1
                - prefix_match: /api
                  id: 2
            "#
        );
        assert!(as_url_path_matched_obj::<TestCallback>(&yaml, None).is_err());

        // Duplicate default
        let yaml = yaml_doc!(
            r#"
                - set_default: true
                  id: 1
                - set_default: true
                  id: 2
            "#
        );
        assert!(as_url_path_matched_obj::<TestCallback>(&yaml, None).is_err());

        // Invalid YAML type
        let yaml = yaml_str!("invalid_type");
        assert!(as_url_path_matched_obj::<TestCallback>(&yaml, None).is_err());

        // Field parse error
        let yaml = yaml_doc!(
            r#"
                prefix_match: /api
                id: "not_an_integer"
            "#
        );
        assert!(as_url_path_matched_obj::<TestCallback>(&yaml, None).is_err());

        // Check failure (id=0)
        let yaml = yaml_doc!(
            r#"
                prefix_match: /api
                id: 0
            "#
        );
        assert!(as_url_path_matched_obj::<TestCallback>(&yaml, None).is_err());
    }
}
