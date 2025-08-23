/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_types::net::Host;
use g3_types::resolve::{PickStrategy, QueryStrategy, ResolveRedirectionBuilder, ResolveStrategy};

const RESOLVE_REDIRECTION_NODE_KEY_EXACT: &str = "exact";
const RESOLVE_REDIRECTION_NODE_KEY_PARENT: &str = "parent";
const RESOLVE_REDIRECTION_NODE_KEY_TO: &str = "to";

pub fn as_query_strategy(v: &Yaml) -> anyhow::Result<QueryStrategy> {
    match v {
        Yaml::String(s) => {
            QueryStrategy::from_str(s).map_err(|_| anyhow!("invalid query strategy string"))
        }
        _ => Err(anyhow!("invalid yaml value type for query strategy")),
    }
}

pub fn as_pick_strategy(v: &Yaml) -> anyhow::Result<PickStrategy> {
    match v {
        Yaml::String(s) => {
            PickStrategy::from_str(s).map_err(|_| anyhow!("invalid pick strategy string"))
        }
        _ => Err(anyhow!("invalid yaml value type for pick strategy")),
    }
}

pub fn as_resolve_strategy(v: &Yaml) -> anyhow::Result<ResolveStrategy> {
    let mut config = ResolveStrategy::default();

    match v {
        Yaml::String(_) => {
            config.query = as_query_strategy(v)?;
            Ok(config)
        }
        Yaml::Hash(map) => {
            crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
                "query" => {
                    config.query = as_query_strategy(v)?;
                    Ok(())
                }
                "pick" => {
                    config.pick = as_pick_strategy(v)?;
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;
            Ok(config)
        }
        _ => Err(anyhow!("invalid yaml value type for resolve strategy")),
    }
}

fn add_exact_redirection_record(
    config: &mut ResolveRedirectionBuilder,
    domain: String,
    v: &Yaml,
) -> anyhow::Result<()> {
    match v {
        Yaml::String(_) => {
            match crate::value::as_host(v).context(format!(
                "invalid resolve redirect host value for domain {domain}",
            ))? {
                Host::Ip(ip) => config.insert_exact(domain, vec![ip]),
                Host::Domain(alias) => config.insert_exact_alias(domain, alias),
            }
            Ok(())
        }
        Yaml::Array(seq) => {
            let mut ips = Vec::with_capacity(seq.len());
            for (i, v) in seq.iter().enumerate() {
                let ip = crate::value::as_ipaddr(v)
                    .context(format!("invalid ip address value for domain {domain}#{i}"))?;
                ips.push(ip);
            }
            config.insert_exact(domain, ips);
            Ok(())
        }
        _ => Err(anyhow!(
            "invalid value type for resolve redirection value of domain {domain}",
        )),
    }
}

fn add_parent_redirection_record(
    config: &mut ResolveRedirectionBuilder,
    parent_domain: String,
    v: &Yaml,
) -> anyhow::Result<()> {
    let to_domain = crate::value::as_domain(v)
        .context("the value should be a domain for parent domain replace")?;
    config.insert_parent(parent_domain, to_domain);
    Ok(())
}

pub fn as_resolve_redirection_builder(v: &Yaml) -> anyhow::Result<ResolveRedirectionBuilder> {
    let mut config = ResolveRedirectionBuilder::default();

    match v {
        Yaml::Hash(map) => {
            crate::foreach_kv(map, |k, v| {
                let domain = idna::domain_to_ascii(k)
                    .map_err(|e| anyhow!("invalid domain to redirect({k}): {e}"))?;
                add_exact_redirection_record(&mut config, domain, v)
            })?;
            Ok(config)
        }
        Yaml::Array(seq) => {
            for (i, v) in seq.iter().enumerate() {
                if let Yaml::Hash(map) = v {
                    let to_v = crate::hash::get_required(map, RESOLVE_REDIRECTION_NODE_KEY_TO)?;
                    if let Ok(exact) =
                        crate::hash::get_required_str(map, RESOLVE_REDIRECTION_NODE_KEY_EXACT)
                    {
                        let domain = idna::domain_to_ascii(exact)
                            .map_err(|e| anyhow!("invalid exact domain in element #{i}: {e}"))?;

                        add_exact_redirection_record(&mut config, domain, to_v).context(
                            format!("invalid exact domain replacement value for element #{i}"),
                        )?;
                    } else if let Ok(parent) =
                        crate::hash::get_required_str(map, RESOLVE_REDIRECTION_NODE_KEY_PARENT)
                    {
                        let parent_domain = idna::domain_to_ascii(parent)
                            .map_err(|e| anyhow!("invalid parent domain in element #{i}: {e}"))?;

                        add_parent_redirection_record(&mut config, parent_domain, to_v).context(
                            format!("invalid parent domain replacement value for element #{i}"),
                        )?;
                    } else {
                        return Err(anyhow!("no exact or parent domain set in element #{i}"));
                    }
                } else {
                    return Err(anyhow!("invalid map value for element #{i}"));
                }
            }
            Ok(config)
        }
        _ => Err(anyhow!("invalid yaml value type for resolve redirection")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g3_types::resolve::ResolveRedirectionValue;
    use std::net::IpAddr;
    use std::sync::Arc;
    use yaml_rust::YamlLoader;

    #[test]
    fn as_query_strategy_ok() {
        // valid strings
        assert_eq!(
            as_query_strategy(&yaml_str!("ipv4only")).unwrap(),
            QueryStrategy::Ipv4Only
        );
        assert_eq!(
            as_query_strategy(&yaml_str!("ipv6only")).unwrap(),
            QueryStrategy::Ipv6Only
        );
        assert_eq!(
            as_query_strategy(&yaml_str!("ipv4first")).unwrap(),
            QueryStrategy::Ipv4First
        );
        assert_eq!(
            as_query_strategy(&yaml_str!("ipv6first")).unwrap(),
            QueryStrategy::Ipv6First
        );
        assert_eq!(
            as_query_strategy(&yaml_str!("ipv4_only")).unwrap(),
            QueryStrategy::Ipv4Only
        );
        assert_eq!(
            as_query_strategy(&yaml_str!("ipv6_first")).unwrap(),
            QueryStrategy::Ipv6First
        );
    }

    #[test]
    fn as_query_strategy_err() {
        // invalid string
        assert!(as_query_strategy(&yaml_str!("invalid")).is_err());

        // non-string type
        assert!(as_query_strategy(&Yaml::Integer(42)).is_err());
        assert!(as_query_strategy(&Yaml::Boolean(true)).is_err());
    }

    #[test]
    fn as_pick_strategy_ok() {
        // valid strings
        assert_eq!(
            as_pick_strategy(&yaml_str!("random")).unwrap(),
            PickStrategy::Random
        );
        assert_eq!(
            as_pick_strategy(&yaml_str!("serial")).unwrap(),
            PickStrategy::Serial
        );
        assert_eq!(
            as_pick_strategy(&yaml_str!("first")).unwrap(),
            PickStrategy::Serial
        );
    }

    #[test]
    fn as_pick_strategy_err() {
        // invalid string
        assert!(as_pick_strategy(&yaml_str!("invalid")).is_err());

        // non-string type
        assert!(as_pick_strategy(&Yaml::Integer(42)).is_err());
        assert!(as_pick_strategy(&Yaml::Boolean(true)).is_err());
    }

    #[test]
    fn as_resolve_strategy_ok() {
        // valid string
        let strategy = as_resolve_strategy(&yaml_str!("ipv4first")).unwrap();
        assert_eq!(strategy.query, QueryStrategy::Ipv4First);
        assert_eq!(strategy.pick, PickStrategy::default());

        // valid hash
        let yaml = yaml_doc!(
            r#"
            query: ipv6first
            pick: serial
            "#
        );
        let strategy = as_resolve_strategy(&yaml).unwrap();
        assert_eq!(strategy.query, QueryStrategy::Ipv6First);
        assert_eq!(strategy.pick, PickStrategy::Serial);
    }

    #[test]
    fn as_resolve_strategy_err() {
        // invalid key
        let yaml = yaml_doc!(
            r#"
            invalid_key: ipv4first
            "#
        );
        assert!(as_resolve_strategy(&yaml).is_err());

        // invalid value type
        assert!(as_resolve_strategy(&Yaml::Integer(42)).is_err());
    }

    #[test]
    fn as_resolve_redirection_builder_ok() {
        // valid hash
        let yaml = yaml_doc!(
            r#"
            example.com: "192.168.1.1"
            example.org: [10.0.0.1, 10.0.0.2]
            alias.com: another.com
            "#
        );
        let builder = as_resolve_redirection_builder(&yaml).unwrap();
        let redirection = builder.build();

        let value = redirection.query_value("example.com").unwrap();
        if let ResolveRedirectionValue::Ip((ipv4, ipv6)) = value {
            assert_eq!(ipv4, vec![IpAddr::from_str("192.168.1.1").unwrap()]);
            assert!(ipv6.is_empty());
        }

        let value = redirection.query_value("example.org").unwrap();
        if let ResolveRedirectionValue::Ip((ipv4, ipv6)) = value {
            assert_eq!(
                ipv4,
                vec![
                    IpAddr::from_str("10.0.0.1").unwrap(),
                    IpAddr::from_str("10.0.0.2").unwrap()
                ]
            );
            assert!(ipv6.is_empty());
        }

        let value = redirection.query_value("alias.com").unwrap();
        if let ResolveRedirectionValue::Domain(alias) = value {
            assert_eq!(alias.as_ref(), "another.com");
        }

        // valid array
        let yaml = yaml_doc!(
            r#"
            - exact: exact1.example.com
              to: 192.168.1.1
            - exact: exact2.example.com
              to: [10.0.0.1, 10.0.0.2]
            - exact: exact3.example.com
              to: alias.domain.com
            - parent: example.com
              to: redirected.com
            "#
        );
        let builder = as_resolve_redirection_builder(&yaml).unwrap();
        let redirection = builder.build();

        let value = redirection.query_value("exact1.example.com").unwrap();
        if let ResolveRedirectionValue::Ip((ipv4, ipv6)) = value {
            assert_eq!(ipv4, vec![IpAddr::from_str("192.168.1.1").unwrap()]);
            assert!(ipv6.is_empty());
        }

        let value = redirection.query_value("exact2.example.com").unwrap();
        if let ResolveRedirectionValue::Ip((ipv4, ipv6)) = value {
            assert_eq!(
                ipv4,
                vec![
                    IpAddr::from_str("10.0.0.1").unwrap(),
                    IpAddr::from_str("10.0.0.2").unwrap()
                ]
            );
            assert!(ipv6.is_empty());
        }

        let value = redirection.query_value("exact3.example.com").unwrap();
        if let ResolveRedirectionValue::Domain(alias) = value {
            assert_eq!(alias.as_ref(), "alias.domain.com");
        }

        let ret = redirection
            .query_first("sub.example.com", QueryStrategy::Ipv4First)
            .unwrap();
        assert_eq!(ret, Host::Domain(Arc::from("sub.redirected.com")));
    }

    #[test]
    fn as_resolve_redirection_builder_err() {
        // invalid YAML type
        let yaml = yaml_str!("invalid");
        assert!(as_resolve_redirection_builder(&yaml).is_err());

        let yaml = Yaml::Integer(42);
        assert!(as_resolve_redirection_builder(&yaml).is_err());

        // array with non-hash element
        let yaml = yaml_doc!(r#"- invalid"#);
        assert!(as_resolve_redirection_builder(&yaml).is_err());

        // array with invalid map element
        let yaml = yaml_doc!(r#"- 42"#);
        assert!(as_resolve_redirection_builder(&yaml).is_err());

        // array with missing required keys (exact or parent)
        let yaml = yaml_doc!(r#"- to: 192.168.1.1"#);
        assert!(as_resolve_redirection_builder(&yaml).is_err());

        // array with no domain set in "parent"
        let yaml = yaml_doc!(
            r#"
            - parent:
              to: redirected.com
            "#
        );
        assert!(as_resolve_redirection_builder(&yaml).is_err());

        // array with invalid domain in "exact"
        let yaml = yaml_doc!(
            r#"
            - exact: null
              to: 192.168.1.1
            "#
        );
        assert!(as_resolve_redirection_builder(&yaml).is_err());

        // array with invalid "to" value for exact (non-string/array)
        let yaml = yaml_doc!(
            r#"
            - exact: example.com
              to: 42
            "#
        );
        assert!(as_resolve_redirection_builder(&yaml).is_err());
    }
}
