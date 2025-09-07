/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::borrow::Cow;

use anyhow::{Context, anyhow};
use yaml_rust::{Yaml, yaml};

use g3_yaml::YamlDocPosition;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct UsernameParamsToEscaperConfig {
    position: Option<YamlDocPosition>,
    /// ordered keys that will be used to form the host label
    pub(crate) keys_for_host: Vec<String>,
    /// require that if a later key appears, all its ancestors (earlier keys) must also appear
    pub(crate) require_hierarchy: bool,
    /// keys that can appear independently without requiring earlier keys (e.g., a generic optional key)
    pub(crate) floating_keys: Vec<String>,
    /// reject unknown keys not present in `keys_for_host`
    pub(crate) reject_unknown_keys: bool,
    /// reject duplicate keys
    pub(crate) reject_duplicate_keys: bool,
    /// separator used between labels
    pub(crate) separator: String,
    /// optional domain suffix appended to computed host (e.g., ".svc.local")
    pub(crate) domain_suffix: Option<String>,
    /// label used when 0 kv pairs (global)
    pub(crate) global_label: String,
    /// default port for HTTP proxy upstream selection
    pub(crate) http_port: u16,
    /// default port for SOCKS5 proxy upstream selection
    pub(crate) socks5_port: u16,
    /// if true, only the base part before '+' is used for auth username
    pub(crate) strip_suffix_for_auth: bool,
}

impl UsernameParamsToEscaperConfig {
    pub(crate) fn new(position: Option<YamlDocPosition>) -> Self {
        UsernameParamsToEscaperConfig {
            position,
            keys_for_host: Vec::new(),
            require_hierarchy: true,
            floating_keys: Vec::new(),
            reject_unknown_keys: true,
            reject_duplicate_keys: true,
            separator: "-".to_string(),
            domain_suffix: None,
            global_label: "global".to_string(),
            http_port: 10000,
            socks5_port: 10001,
            strip_suffix_for_auth: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yaml_rust::YamlLoader;

    #[test]
    fn parse_minimal_and_aliases() {
        let s = r#"
        username_params_to_escaper_addr:
          keys: [a, b, c]
          floating: [c]
          require_hierarchy: false
          reject_unknown_keys: false
          reject_duplicate_keys: false
          separator: ":"
          suffix: "svc.local"
          global_label: "g"
          http_port: 20000
          socks_port: 20001
          auth_strip_suffix: false
        "#;
        let docs = YamlLoader::load_from_str(s).unwrap();
        let root = &docs[0]["username_params_to_escaper_addr"];
        let map = root.as_hash().unwrap();
        let c = UsernameParamsToEscaperConfig::parse(map, None).unwrap();

        assert_eq!(c.keys_for_host, vec!["a", "b", "c"]);
        assert_eq!(c.floating_keys, vec!["c"]);
        assert!(!c.require_hierarchy);
        assert!(!c.reject_unknown_keys);
        assert!(!c.reject_duplicate_keys);
        assert_eq!(c.separator, ":");
        assert_eq!(c.domain_suffix.as_deref(), Some(".svc.local"));
        assert_eq!(c.global_label, "g");
        assert_eq!(c.http_port, 20000);
        assert_eq!(c.socks5_port, 20001);
        assert!(!c.strip_suffix_for_auth);
    }

    #[test]
    fn to_fqdn_works() {
        let mut c = UsernameParamsToEscaperConfig::new(None);
        // no suffix
        assert_eq!(c.to_fqdn("foo").as_ref(), "foo");

        // with suffix
        c.domain_suffix = Some(".example".to_string());
        assert_eq!(c.to_fqdn("foo").as_ref(), "foo.example");

        // suffix normalized from non-dot value
        let s = r#"
        username_params_to_escaper_addr:
          keys_for_host: [x]
          domain_suffix: "svc.local"
        "#;
        let docs = YamlLoader::load_from_str(s).unwrap();
        let map = docs[0]["username_params_to_escaper_addr"].as_hash().unwrap();
        let c2 = UsernameParamsToEscaperConfig::parse(map, None).unwrap();
        assert_eq!(c2.domain_suffix.as_deref(), Some(".svc.local"));
    }

    #[test]
    fn invalid_floating_key_rejected() {
        let s = r#"
        username_params_to_escaper_addr:
          keys_for_host: [a]
          floating_keys: [b]
        "#;
        let docs = YamlLoader::load_from_str(s).unwrap();
        let map = docs[0]["username_params_to_escaper_addr"].as_hash().unwrap();
        let r = UsernameParamsToEscaperConfig::parse(map, None);
        assert!(r.is_err());
    }
}

impl UsernameParamsToEscaperConfig {
    pub(crate) fn parse(map: &yaml::Hash, position: Option<YamlDocPosition>) -> anyhow::Result<Self> {
        let mut c = Self::new(position);
        g3_yaml::foreach_kv(map, |k, v| c.set(k, v))?;
        c.check()?;
        Ok(c)
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            "keys_for_host" | "keys" => {
                self.keys_for_host = g3_yaml::value::as_list(v, |v| g3_yaml::value::as_string(v))
                    .context(format!("invalid string list value for key {k}"))?;
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
                self.floating_keys = g3_yaml::value::as_list(v, |v| g3_yaml::value::as_string(v))
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
                if !s.is_empty() && !s.starts_with('.') {
                    s.insert(0, '.');
                }
                self.domain_suffix = Some(s);
                Ok(())
            }
            "global_label" => {
                self.global_label = g3_yaml::value::as_string(v)?;
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
        if self.keys_for_host.iter().any(|k| k.is_empty()) {
            return Err(anyhow!("keys_for_host contains empty key"));
        }
        // allow empty separator when only one label or when explicitly desired
        // ensure floating keys are included in keys_for_host
        for fk in &self.floating_keys {
            if !self.keys_for_host.iter().any(|k| k == fk) {
                return Err(anyhow!("floating key {fk} must be listed in keys_for_host"));
            }
        }
        Ok(())
    }

    pub(crate) fn to_fqdn<'a>(&'a self, host: &'a str) -> Cow<'a, str> {
        if let Some(sfx) = &self.domain_suffix {
            if sfx.is_empty() {
                Cow::Borrowed(host)
            } else {
                let mut s = String::with_capacity(host.len() + sfx.len());
                s.push_str(host);
                s.push_str(sfx);
                Cow::Owned(s)
            }
        } else {
            Cow::Borrowed(host)
        }
    }
}
