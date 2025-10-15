/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, anyhow};
use itertools::Itertools;
use yaml_rust::Yaml;

use g3_types::net::{Host, UpstreamAddr};

use crate::escape::EgressUpstream;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UsernameParamsConfig {
    known_keys: BTreeSet<String>,
    /// ordered keys that will be used to form the host label
    keys_for_host: Vec<String>,
    /// Sticky key for domain resolve
    resolve_sticky_key: String,
    /// require that if a later key appears, all its ancestors (earlier keys) must also appear
    require_hierarchy: bool,
    /// keys that can appear independently without requiring earlier keys (e.g., a generic optional key)
    floating_keys: Vec<String>,
    /// reject unknown keys not present in `keys_for_host`
    reject_unknown_keys: bool,
    /// reject duplicate keys
    reject_duplicate_keys: bool,
    /// separator used between labels
    separator: String,
    /// optional domain suffix appended to computed host (e.g., ".svc.local")
    domain_suffix: String,
    /// default port for HTTP proxy upstream selection
    http_port: u16,
    /// default port for SOCKS5 proxy upstream selection
    socks5_port: u16,
    /// if true, only the base part before '+' is used for auth username
    strip_suffix_for_auth: bool,
}

impl UsernameParamsConfig {
    pub(crate) fn new() -> Self {
        UsernameParamsConfig {
            known_keys: BTreeSet::new(),
            keys_for_host: Vec::new(),
            resolve_sticky_key: String::new(),
            require_hierarchy: true,
            floating_keys: Vec::new(),
            reject_unknown_keys: true,
            reject_duplicate_keys: true,
            separator: "-".to_string(),
            domain_suffix: String::new(),
            http_port: 10000,
            socks5_port: 10001,
            strip_suffix_for_auth: true,
        }
    }
}

impl UsernameParamsConfig {
    pub(crate) fn parse(value: &Yaml) -> anyhow::Result<Self> {
        if let Yaml::Hash(map) = value {
            let mut c = Self::new();
            g3_yaml::foreach_kv(map, |k, v| c.set(k, v))?;
            c.check()?;
            Ok(c)
        } else {
            Err(anyhow!(
                "Yaml value type for `UsernameParamsToEscaperConfig` should be map"
            ))
        }
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            "keys_for_host" | "keys" => {
                self.keys_for_host = g3_yaml::value::as_list(v, g3_yaml::value::as_string)
                    .context(format!("invalid string list value for key {k}"))?;
                Ok(())
            }
            "resolve_sticky_key" => {
                self.resolve_sticky_key = g3_yaml::value::as_string(v)?;
                Ok(())
            }
            "require_hierarchy" => {
                self.require_hierarchy = g3_yaml::value::as_bool(v)?;
                Ok(())
            }
            "reject_unknown_keys" => {
                self.reject_unknown_keys = g3_yaml::value::as_bool(v)?;
                Ok(())
            }
            "floating_keys" | "floating" => {
                self.floating_keys = g3_yaml::value::as_list(v, g3_yaml::value::as_string)
                    .context(format!("invalid string list value for key {k}"))?;
                Ok(())
            }
            "reject_duplicate_keys" => {
                self.reject_duplicate_keys = g3_yaml::value::as_bool(v)?;
                Ok(())
            }
            "separator" => {
                self.separator = g3_yaml::value::as_string(v)?;
                Ok(())
            }
            "domain_suffix" | "suffix" => {
                let mut s = g3_yaml::value::as_string(v)?;
                if !s.starts_with('.') {
                    s.insert(0, '.');
                }
                self.domain_suffix = s;
                Ok(())
            }
            "http_port" => {
                self.http_port = g3_yaml::value::as_u16(v)
                    .context(format!("invalid u16 port value for key {k}"))?;
                Ok(())
            }
            "socks5_port" | "socks_port" => {
                self.socks5_port = g3_yaml::value::as_u16(v)
                    .context(format!("invalid u16 port value for key {k}"))?;
                Ok(())
            }
            "strip_suffix_for_auth" | "auth_strip_suffix" => {
                self.strip_suffix_for_auth = g3_yaml::value::as_bool(v)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }

    fn check(&mut self) -> anyhow::Result<()> {
        for k in &self.keys_for_host {
            if k.is_empty() {
                return Err(anyhow!("keys_for_host contains empty key"));
            }
            self.known_keys.insert(k.to_string());
        }

        // allow empty separator when only one label or when explicitly desired
        // ensure floating keys are included in keys_for_host
        for k in &self.floating_keys {
            if !self.known_keys.contains(k) {
                return Err(anyhow!("floating key {k} must be listed in keys_for_host"));
            }
        }

        if !self.resolve_sticky_key.is_empty() {
            self.known_keys.insert(self.resolve_sticky_key.clone());
        }
        Ok(())
    }

    pub(crate) fn real_username<'a>(&self, raw: &'a str) -> &'a str {
        if self.strip_suffix_for_auth
            && let Some(p) = memchr::memchr(b'+', raw.as_bytes())
        {
            &raw[..p]
        } else {
            raw
        }
    }

    pub(crate) fn parse_egress_upstream_socks5(
        &self,
        raw: &str,
    ) -> anyhow::Result<Option<EgressUpstream>> {
        self.parse_egress_upstream(raw, self.socks5_port)
    }

    pub(crate) fn parse_egress_upstream_http(
        &self,
        raw: &str,
    ) -> anyhow::Result<Option<EgressUpstream>> {
        self.parse_egress_upstream(raw, self.http_port)
    }

    fn parse_param<'a>(&self, param: &'a str) -> anyhow::Result<(&'a str, &'a str)> {
        let Some(p) = memchr::memchr(b'=', param.as_bytes()) else {
            return Err(anyhow!("no '=' found in username param {param}"));
        };
        let k = &param[..p];
        let v = &param[p + 1..];
        if k.is_empty() || v.is_empty() {
            return Err(anyhow!("empty fields in username param {param}"));
        }
        if self.reject_unknown_keys && !self.known_keys.contains(k) {
            return Err(anyhow!("unknown username param {param}"));
        }
        Ok((k, v))
    }

    fn parse_egress_upstream(
        &self,
        raw: &str,
        port: u16,
    ) -> anyhow::Result<Option<EgressUpstream>> {
        let Some(p) = memchr::memchr(b'+', raw.as_bytes()) else {
            return Ok(None);
        };
        let mut left = &raw[p + 1..];

        let mut params = BTreeMap::new();
        while let Some(p) = memchr::memchr(b'+', left.as_bytes()) {
            let param = &left[..p];
            left = &left[p + 1..];

            let (k, v) = self.parse_param(param)?;
            let old_v = params.insert(k, v);
            if old_v.is_some() && self.reject_duplicate_keys {
                return Err(anyhow!("duplicate key in username param {param}"));
            }
        }
        if !left.is_empty() {
            let (k, v) = self.parse_param(left)?;
            let old_v = params.insert(k, v);
            if old_v.is_some() && self.reject_duplicate_keys {
                return Err(anyhow!("duplicate key in username param {left}"));
            }
        }

        if params.is_empty() {
            return Ok(None);
        }

        if self.require_hierarchy {
            // if a later non-floating key appears, all earlier non-floating must be present
            let mut saw_missing_required = false;
            for key in &self.keys_for_host {
                if self.floating_keys.contains(key) {
                    continue;
                }
                if params.contains_key(key.as_str()) {
                    if saw_missing_required {
                        return Err(anyhow!(
                            "key {key} requires its ancestor keys to be present"
                        ));
                    }
                } else {
                    // mark that following present required keys will violate hierarchy
                    saw_missing_required = true;
                }
            }
        }

        let mut host = self
            .keys_for_host
            .iter()
            .filter_map(|k| params.get(k.as_str()))
            .copied()
            .join(&self.separator);
        if host.is_empty() {
            return Ok(None);
        }

        let resolve_sticky_key = if self.resolve_sticky_key.is_empty() {
            String::new()
        } else {
            params
                .get(self.resolve_sticky_key.as_str())
                .map(|v| (*v).to_string())
                .unwrap_or_default()
        };

        if !self.domain_suffix.is_empty() {
            host.push_str(&self.domain_suffix);
        }
        if host.len() == 9 {
            if host == "localhost" || host == "LOCALHOST" {
                return Ok(Some(EgressUpstream {
                    addr: UpstreamAddr::new(Host::localhost_v4(), port),
                    resolve_sticky_key,
                }));
            }
        } else if host.len() > 9 && (host.ends_with(".localhost") || host.ends_with(".LOCALHOST")) {
            return Ok(Some(EgressUpstream {
                addr: UpstreamAddr::new(Host::localhost_v4(), port),
                resolve_sticky_key,
            }));
        }

        let addr = UpstreamAddr::from_host_str_and_port(&host, port)?;
        Ok(Some(EgressUpstream {
            addr,
            resolve_sticky_key,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g3_yaml::yaml_doc;
    use yaml_rust::YamlLoader;

    #[test]
    fn parse_minimal_and_aliases() {
        let value = yaml_doc!(
            r#"
                keys: [a, b, c]
                floating: [c]
                require_hierarchy: false
                reject_unknown_keys: false
                reject_duplicate_keys: false
                separator: ":"
                suffix: "svc.local"
                http_port: 20000
                socks_port: 20001
                auth_strip_suffix: false
            "#
        );
        let c = UsernameParamsConfig::parse(&value).unwrap();

        assert_eq!(c.keys_for_host, vec!["a", "b", "c"]);
        assert_eq!(c.floating_keys, vec!["c"]);
        assert!(!c.require_hierarchy);
        assert!(!c.reject_unknown_keys);
        assert!(!c.reject_duplicate_keys);
        assert_eq!(c.separator, ":");
        assert_eq!(c.domain_suffix, ".svc.local");
        assert_eq!(c.http_port, 20000);
        assert_eq!(c.socks5_port, 20001);
        assert!(!c.strip_suffix_for_auth);
    }

    #[test]
    fn invalid_floating_key_rejected() {
        let value = yaml_doc!(
            r#"
                keys_for_host: [a]
                floating_keys: [b]
            "#
        );
        let r = UsernameParamsConfig::parse(&value);
        assert!(r.is_err());
    }

    #[test]
    fn parse_valid() {
        let value = yaml_doc!(
            r#"
                keys_for_host: ["label1", "label2"]
                domain-suffix: example.net
                resolve_sticky_key: label3
                http_port: 8080
                socks_port: 1080
            "#
        );
        let c = UsernameParamsConfig::parse(&value).unwrap();

        let ups = c
            .parse_egress_upstream_http("test+label1=foo+label2=bar")
            .unwrap()
            .unwrap();
        assert_eq!(ups.addr.port(), 8080);
        assert_eq!(ups.addr.host().to_string(), "foo-bar.example.net");
        assert!(ups.resolve_sticky_key.is_empty());

        let ups = c
            .parse_egress_upstream_socks5("test+label1=foo+label2=bar")
            .unwrap()
            .unwrap();
        assert_eq!(ups.addr.port(), 1080);
        assert_eq!(ups.addr.host().to_string(), "foo-bar.example.net");
        assert!(ups.resolve_sticky_key.is_empty());

        let ups = c
            .parse_egress_upstream("test+label1=foo+label2=bar+label3=data", 80)
            .unwrap()
            .unwrap();
        assert_eq!(ups.addr.port(), 80);
        assert_eq!(ups.addr.host().to_string(), "foo-bar.example.net");
        assert_eq!(ups.resolve_sticky_key, "data");
    }

    #[test]
    fn parse_allow_duplicate_key() {
        let value = yaml_doc!(
            r#"
                keys_for_host: ["label1", "label2"]
                reject_duplicate_keys: false
            "#
        );
        let c = UsernameParamsConfig::parse(&value).unwrap();

        let ups = c
            .parse_egress_upstream("test+label1=foo+label2=bar+label2=foo", 80)
            .unwrap()
            .unwrap();
        assert_eq!(ups.addr.host().to_string(), "foo-foo");
    }

    #[test]
    fn parse_reject_duplicate_key() {
        let value = yaml_doc!(
            r#"
                keys_for_host: ["label1", "label2"]
                reject_duplicate_keys: true
            "#
        );
        let c = UsernameParamsConfig::parse(&value).unwrap();

        assert!(
            c.parse_egress_upstream("test+label1=foo+label2=bar+label2=foo", 80)
                .is_err()
        );
    }

    #[test]
    fn parse_allow_unknown_key() {
        let value = yaml_doc!(
            r#"
                keys_for_host: ["label1", "label2"]
                reject_unknown_keys: false
            "#
        );
        let c = UsernameParamsConfig::parse(&value).unwrap();

        let ups = c
            .parse_egress_upstream("test+label1=foo+label2=bar+label3=foo", 80)
            .unwrap()
            .unwrap();
        assert_eq!(ups.addr.host().to_string(), "foo-bar");

        // no known keys, return Ok(None)
        assert!(
            c.parse_egress_upstream("test+label3=foo", 80)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn parse_reject_unknown_key() {
        let value = yaml_doc!(
            r#"
                keys_for_host: ["label1", "label2"]
                reject_unknown_keys: true
            "#
        );
        let c = UsernameParamsConfig::parse(&value).unwrap();

        assert!(
            c.parse_egress_upstream("test+label1=foo+label2=bar+label3=foo", 80)
                .is_err()
        );
    }

    #[test]
    fn parse_require_hierarchy() {
        let value = yaml_doc!(
            r#"
                keys_for_host: ["label1", "label2"]
                require_hierarchy: true
            "#
        );
        let c = UsernameParamsConfig::parse(&value).unwrap();

        let ups = c
            .parse_egress_upstream("test+label1=foo+label2=bar", 80)
            .unwrap()
            .unwrap();
        assert_eq!(ups.addr.host().to_string(), "foo-bar");

        assert!(c.parse_egress_upstream("test+label2=bar", 80).is_err());
    }

    #[test]
    fn parse_no_require_hierarchy() {
        let value = yaml_doc!(
            r#"
                keys_for_host: ["label1", "label2"]
                require_hierarchy: false
            "#
        );
        let c = UsernameParamsConfig::parse(&value).unwrap();

        let ups = c
            .parse_egress_upstream("test+label2=bar", 80)
            .unwrap()
            .unwrap();
        assert_eq!(ups.addr.host().to_string(), "bar");
    }

    #[test]
    fn parse_with_floating_keys() {
        let value = yaml_doc!(
            r#"
                keys_for_host: ["label1", "label2", "opt"]
                floating_keys: ["opt"]
                require_hierarchy: true
            "#
        );
        let c = UsernameParamsConfig::parse(&value).unwrap();

        let ups = c
            .parse_egress_upstream("test+opt=o123", 80)
            .unwrap()
            .unwrap();
        assert_eq!(ups.addr.host().to_string(), "o123");

        let ups = c
            .parse_egress_upstream("test+label1=foo+opt=o123", 80)
            .unwrap()
            .unwrap();
        assert_eq!(ups.addr.host().to_string(), "foo-o123");

        let ups = c
            .parse_egress_upstream("test+label1=foo+label2=bar+opt=o123", 80)
            .unwrap()
            .unwrap();
        assert_eq!(ups.addr.host().to_string(), "foo-bar-o123");

        assert!(
            c.parse_egress_upstream("test+label2=bar+opt=o123", 80)
                .is_err()
        );

        // missing key but not require hierarchy
        let value = yaml_doc!(
            r#"
                keys_for_host: ["label1", "label2", "opt"]
                floating_keys: ["opt"]
                require_hierarchy: false
            "#
        );
        let c = UsernameParamsConfig::parse(&value).unwrap();
        let ups = c
            .parse_egress_upstream("test+label2=bar+opt=o123", 80)
            .unwrap()
            .unwrap();
        assert_eq!(ups.addr.host().to_string(), "bar-o123");

        // missing key but it's not required
        let value = yaml_doc!(
            r#"
                keys_for_host: ["label1", "label2", "opt"]
                floating_keys: ["label1", "opt"]
                require_hierarchy: true
            "#
        );
        let c = UsernameParamsConfig::parse(&value).unwrap();
        let ups = c
            .parse_egress_upstream("test+label2=bar+opt=o123", 80)
            .unwrap()
            .unwrap();
        assert_eq!(ups.addr.host().to_string(), "bar-o123");
    }

    #[test]
    fn parse_resolve_localhost() {
        let value = yaml_doc!(
            r#"
                keys_for_host: ["label1", "label2"]
            "#
        );
        let c = UsernameParamsConfig::parse(&value).unwrap();

        let ups = c
            .parse_egress_upstream_http("test+label1=localhost")
            .unwrap()
            .unwrap();
        assert_eq!(ups.addr.host().to_string(), "127.0.0.1");

        let ups = c
            .parse_egress_upstream_http("test+label1=LOCALHOST")
            .unwrap()
            .unwrap();
        assert_eq!(ups.addr.host().to_string(), "127.0.0.1");

        let ups = c
            .parse_egress_upstream_http("test+label1=a.localhost")
            .unwrap()
            .unwrap();
        assert_eq!(ups.addr.host().to_string(), "127.0.0.1");

        let ups = c
            .parse_egress_upstream_http("test+label1=a.LOCALHOST")
            .unwrap()
            .unwrap();
        assert_eq!(ups.addr.host().to_string(), "127.0.0.1");
    }
}
