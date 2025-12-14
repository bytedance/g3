/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_types::net::Host;
use g3_types::route::HostMatch;

use crate::{YamlDocPosition, YamlMapCallback};

fn add_host_matched_value<T: YamlMapCallback>(
    obj: &mut HostMatch<Arc<T>>,
    value: &Yaml,
    mut target: T,
    doc: Option<&YamlDocPosition>,
) -> anyhow::Result<()> {
    let type_name = target.type_name();

    if let Yaml::Hash(map) = value {
        let mut exact_ip_vs = vec![];
        let mut exact_domain_vs = vec![];
        let mut child_domain_vs = vec![];
        let mut set_default = false;

        let mut add_exact_host_match_value = |v: &Yaml| -> anyhow::Result<()> {
            match crate::value::as_host(v)? {
                Host::Ip(ip) => exact_ip_vs.push(ip),
                Host::Domain(domain) => exact_domain_vs.push(domain),
            }
            Ok(())
        };

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "set_default" => {
                set_default =
                    crate::value::as_bool(v).context(format!("invalid bool value for key {k}"))?;
                Ok(())
            }
            "exact_match" => {
                if let Yaml::Array(seq) = v {
                    for (i, v) in seq.iter().enumerate() {
                        add_exact_host_match_value(v)
                            .context(format!("invalid host string value for {k}#{i}"))?;
                    }
                } else {
                    add_exact_host_match_value(v)
                        .context(format!("invalid host string value for key {k}"))?;
                }
                Ok(())
            }
            "child_match" => {
                if let Yaml::Array(seq) = v {
                    for (i, v) in seq.iter().enumerate() {
                        let domain = crate::value::as_domain(v)
                            .context(format!("invalid domain string value for {k}#{i}"))?;
                        child_domain_vs.push(domain);
                    }
                } else {
                    let domain = crate::value::as_domain(v)
                        .context(format!("invalid domain string value for key {k}"))?;
                    child_domain_vs.push(domain);
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
        for ip in exact_ip_vs {
            if obj.add_exact_ip(ip, Arc::clone(&t)).is_some() {
                return Err(anyhow!("duplicate {type_name} value for host ip {ip}"));
            }
            auto_default = false;
        }
        for domain in &exact_domain_vs {
            if obj
                .add_exact_domain(domain.clone(), Arc::clone(&t))
                .is_some()
            {
                return Err(anyhow!(
                    "duplicate {type_name} value for host domain {domain}"
                ));
            }
            auto_default = false;
        }
        for domain in &child_domain_vs {
            if obj.add_child_domain(domain, Arc::clone(&t)).is_some() {
                return Err(anyhow!(
                    "duplicate {type_name} value for child domain {domain}"
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
            "yaml type for 'host matched {type_name} value' should be 'map'"
        ))
    }
}

pub fn as_host_matched_obj<T>(
    value: &Yaml,
    doc: Option<&YamlDocPosition>,
) -> anyhow::Result<HostMatch<Arc<T>>>
where
    T: Default + YamlMapCallback,
{
    let mut obj = HostMatch::<Arc<T>>::default();

    if let Yaml::Array(seq) = value {
        for (i, v) in seq.iter().enumerate() {
            let target = T::default();
            let type_name = target.type_name();
            add_host_matched_value(&mut obj, v, target, doc).context(format!(
                "invalid host matched {type_name} value for element #{i}"
            ))?;
        }
    } else {
        let target = T::default();
        let type_name = target.type_name();
        add_host_matched_value(&mut obj, value, target, doc)
            .context(format!("invalid host matched {type_name} value"))?;
    }

    Ok(obj)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::IpAddr;
    use std::str::FromStr;
    use yaml_rust::YamlLoader;

    // Define a test struct implementing YamlMapCallback
    #[derive(Default)]
    struct TestCallback {
        name: String,
        value: i32,
    }

    impl YamlMapCallback for TestCallback {
        fn type_name(&self) -> &'static str {
            "TestCallback"
        }

        fn parse_kv(
            &mut self,
            key: &str,
            value: &Yaml,
            _doc: Option<&YamlDocPosition>,
        ) -> anyhow::Result<()> {
            match key {
                "name" => {
                    self.name = value.as_str().ok_or(anyhow!("invalid string"))?.to_string();
                    Ok(())
                }
                "value" => {
                    self.value = value.as_i64().ok_or(anyhow!("invalid integer"))? as i32;
                    Ok(())
                }
                _ => Err(anyhow!("unknown key: {}", key)),
            }
        }

        fn check(&mut self) -> anyhow::Result<()> {
            if self.name.is_empty() {
                Err(anyhow!("name is required"))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn as_host_matched_obj_ok() {
        // Single map with exact IP match
        let yaml = yaml_doc!(
            r#"
                exact_match: 192.168.0.1
                name: test1
                value: 100
            "#
        );
        let host_match: HostMatch<Arc<TestCallback>> = as_host_matched_obj(&yaml, None).unwrap();
        let ip = IpAddr::from_str("192.168.0.1").unwrap();
        let value = host_match.get(&Host::Ip(ip)).unwrap();
        assert_eq!(value.name, "test1");
        assert_eq!(value.value, 100);
        assert!(host_match.get_default().is_none());

        // Single map with domain match
        let yaml = yaml_doc!(
            r#"
                child_match: example.com
                name: test2
                value: 200
            "#
        );
        let host_match: HostMatch<Arc<TestCallback>> = as_host_matched_obj(&yaml, None).unwrap();
        let domain = Host::Domain("example.com".into());
        let value = host_match.get(&domain).unwrap();
        assert_eq!(value.name, "test2");
        assert_eq!(value.value, 200);
        assert!(host_match.get_default().is_none());

        // Single map with default
        let yaml = yaml_doc!(
            r#"
                set_default: true
                name: default
                value: 0
            "#
        );
        let host_match: HostMatch<Arc<TestCallback>> = as_host_matched_obj(&yaml, None).unwrap();
        let default_value = host_match.get_default().unwrap();
        assert_eq!(default_value.name, "default");
        assert_eq!(default_value.value, 0);
        let ip = IpAddr::from_str("192.168.0.1").unwrap();
        let value = host_match.get(&Host::Ip(ip)).unwrap();
        assert_eq!(value.name, "default");
        assert_eq!(value.value, 0);

        // Array of maps
        let yaml = yaml_doc!(
            r#"
                - exact_match: 192.168.0.1
                  name: test1
                  value: 100
                - child_match: example.com
                  name: test2
                  value: 200
                - set_default: true
                  name: default
                  value: 0
            "#
        );
        let host_match: HostMatch<Arc<TestCallback>> = as_host_matched_obj(&yaml, None).unwrap();
        let ip = IpAddr::from_str("192.168.0.1").unwrap();
        let value = host_match.get(&Host::Ip(ip)).unwrap();
        assert_eq!(value.name, "test1");
        assert_eq!(value.value, 100);
        let domain = Host::Domain("example.com".into());
        let value = host_match.get(&domain).unwrap();
        assert_eq!(value.name, "test2");
        assert_eq!(value.value, 200);
        let default_value = host_match.get_default().unwrap();
        assert_eq!(default_value.name, "default");
        assert_eq!(default_value.value, 0);
        let other_ip = IpAddr::from_str("10.0.0.1").unwrap();
        let value = host_match.get(&Host::Ip(other_ip)).unwrap();
        assert_eq!(value.name, "default");
        assert_eq!(value.value, 0);

        // Exact match as array
        let yaml = yaml_doc!(
            r#"
                exact_match:
                  - 192.168.0.1
                  - 10.0.0.1
                name: test
                value: 100
            "#
        );
        let host_match: HostMatch<Arc<TestCallback>> =
            as_host_matched_obj::<TestCallback>(&yaml, None).unwrap();
        let ip1 = IpAddr::from_str("192.168.0.1").unwrap();
        let value1 = host_match.get(&Host::Ip(ip1)).unwrap();
        assert_eq!(value1.name, "test");
        assert_eq!(value1.value, 100);
        let ip2 = IpAddr::from_str("10.0.0.1").unwrap();
        let value2 = host_match.get(&Host::Ip(ip2)).unwrap();
        assert_eq!(value2.name, "test");
        assert_eq!(value2.value, 100);

        // Child match as array
        let yaml = yaml_doc!(
            r#"
                child_match:
                  - example.com
                  - test.org
                name: test
                value: 100
            "#
        );
        let host_match: HostMatch<Arc<TestCallback>> =
            as_host_matched_obj::<TestCallback>(&yaml, None).unwrap();
        let domain1 = Host::Domain("example.com".into());
        let value1 = host_match.get(&domain1).unwrap();
        assert_eq!(value1.name, "test");
        assert_eq!(value1.value, 100);
        let domain2 = Host::Domain("test.org".into());
        let value2 = host_match.get(&domain2).unwrap();
        assert_eq!(value2.name, "test");
        assert_eq!(value2.value, 100);
    }

    #[test]
    fn as_host_matched_obj_err() {
        // Invalid YAML type
        let yaml = yaml_str!("not a map or array");
        assert!(as_host_matched_obj::<TestCallback>(&yaml, None).is_err());

        // Duplicate exact IP match
        let yaml = yaml_doc!(
            r#"
                - exact_match: 192.168.0.1
                  name: test1
                  value: 100
                - exact_match: 192.168.0.1
                  name: test2
                  value: 200
            "#
        );
        assert!(as_host_matched_obj::<TestCallback>(&yaml, None).is_err());

        // Duplicate child domain match
        let yaml = yaml_doc!(
            r#"
                - child_match: example.com
                  name: test1
                  value: 100
                - child_match: example.com
                  name: test2
                  value: 200
            "#
        );
        assert!(as_host_matched_obj::<TestCallback>(&yaml, None).is_err());

        // Multiple defaults
        let yaml = yaml_doc!(
            r#"
                - set_default: true
                  name: default1
                  value: 0
                - set_default: true
                  name: default2
                  value: 1
            "#
        );
        assert!(as_host_matched_obj::<TestCallback>(&yaml, None).is_err());

        // Missing host string
        let yaml = yaml_doc!(
            r#"
                exact_match:
                name: test
                value: 100
            "#
        );
        assert!(as_host_matched_obj::<TestCallback>(&yaml, None).is_err());

        // Missing required fields
        let yaml = yaml_doc!(
            r#"
                exact_match: 192.168.0.1
                value: 100
            "#
        );
        assert!(as_host_matched_obj::<TestCallback>(&yaml, None).is_err());
    }
}
