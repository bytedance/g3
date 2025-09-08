/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::collections::HashMap;

use anyhow::anyhow;
use log::debug;

use g3_types::net::UpstreamAddr;

use crate::config::server::username_params_to_escaper::UsernameParamsToEscaperConfig;

#[derive(Debug, Clone)]
pub(crate) struct ParsedUsernameParams {
    pub(crate) base: String,
    pub(crate) params: HashMap<String, String>,
}

impl ParsedUsernameParams {
    pub(crate) fn parse(original: &str) -> anyhow::Result<Self> {
        // expected format: base[+key=value]*, keys/values are non-empty, no escaping
        let mut it = original.split('+');
        let base = it
            .next()
            .ok_or_else(|| anyhow!("empty username"))?
            .to_string();
        let mut params = HashMap::new();
        for t in it {
            let mut kv = t.splitn(2, '=');
            let k = kv
                .next()
                .ok_or_else(|| anyhow!("invalid token: missing key"))?;
            let v = kv
                .next()
                .ok_or_else(|| anyhow!("invalid token: missing value for key {}", k))?;
            if k.is_empty() || v.is_empty() {
                return Err(anyhow!("empty key or value in username params"));
            }
            if params.contains_key(k) {
                return Err(anyhow!("duplicate key {}", k));
            }
            params.insert(k.to_string(), v.to_string());
        }
        Ok(ParsedUsernameParams { base, params })
    }

    pub(crate) fn auth_base(original: &str) -> &str {
        // return substring before first '+'
        if let Some((base, _)) = original.split_once('+') {
            base
        } else {
            original
        }
    }
}

#[derive(Copy, Clone, Debug)]
pub(crate) enum InboundKind {
    Http,
    Socks5,
}

// Quick check to decide whether a username contains at least one known key pattern
// without fully parsing. This is used to decide if we should attempt mapping.
pub(crate) fn username_has_known_key(
    cfg: &UsernameParamsToEscaperConfig,
    username_original: &str,
) -> bool {
    if !username_original.contains('+') {
        return false;
    }
    for k in &cfg.keys_for_host {
        // look for "+key=" pattern to avoid false positives
        let mut pat = String::with_capacity(k.len() + 2);
        pat.push('+');
        pat.push_str(k);
        pat.push('=');
        if username_original.contains(&pat) {
            return true;
        }
    }
    false
}

pub(crate) fn compute_upstream_from_username(
    cfg: &UsernameParamsToEscaperConfig,
    username_original: &str,
    inbound: InboundKind,
) -> anyhow::Result<UpstreamAddr> {
    debug!(
        "username-params: inbound={:?} original_len={}",
        inbound,
        username_original.len()
    );
    let parsed = ParsedUsernameParams::parse(username_original)?;
    debug!(
        "username-params: base='{}' params={:?}",
        parsed.base, parsed.params
    );

    // validate keys
    if cfg.reject_unknown_keys {
        for k in parsed.params.keys() {
            if !cfg.keys_for_host.contains(k) {
                debug!("username-params: reject unknown key '{}'", k.as_str());
                return Err(anyhow!("unknown key {}", k.as_str()));
            }
        }
    }

    if cfg.require_hierarchy {
        // if a later non-floating key appears, all earlier non-floating must be present
        let mut saw_missing_required = false;
        for key in &cfg.keys_for_host {
            let key_name = key.as_str().to_owned();
            let is_floating = cfg.floating_keys.contains(key);
            match parsed.params.get(key_name.as_str()) {
                Some(_v) => {
                    if saw_missing_required && !is_floating {
                        debug!(
                            "username-params: hierarchy violation at key '{}' (floating={})",
                            key_name.as_str(),
                            is_floating
                        );
                        return Err(anyhow!(
                            "key {} requires its ancestor keys to be present",
                            key_name.as_str()
                        ));
                    }
                }
                None => {
                    if !is_floating {
                        // mark that following present required keys will violate hierarchy
                        saw_missing_required = true;
                    }
                }
            }
        }
    }

    // build host label sequence in configured order, skipping missing keys
    let mut parts: Vec<&str> = Vec::new();
    for k in &cfg.keys_for_host {
        if let Some(v) = parsed.params.get(k) {
            parts.push(v);
        }
    }
    debug!(
        "username-params: keys_for_host={:?} floating_keys={:?} used_parts={:?}",
        cfg.keys_for_host, cfg.floating_keys, parts
    );

    let port = match inbound {
        InboundKind::Http => cfg.http_port,
        InboundKind::Socks5 => cfg.socks5_port,
    };

    // Build label or use global
    if parts.is_empty() {
        // No effective known keys provided; let caller fall back to escaper defaults (no override)
        return Err(anyhow!("no known keys provided in username params"));
    }

    // join with the configured separator
    let mut label = String::new();
    for (i, v) in parts.iter().enumerate() {
        if i > 0 {
            label.push_str(&cfg.separator);
        }
        label.push_str(v);
    }
    debug!(
        "username-params: joined label='{}' separator='{}'",
        label, cfg.separator
    );

    // Apply optional suffix
    let full_host_cow = cfg.to_fqdn(&label);
    let full_host = full_host_cow.as_ref();
    if !matches!(full_host_cow, std::borrow::Cow::Borrowed(_)) {
        debug!(
            "username-params: apply domain_suffix -> host='{}'",
            full_host
        );
    } else {
        debug!("username-params: no domain_suffix -> host='{}'", full_host);
    }
    debug!("username-params: chosen port={} from inbound={:?}", port, inbound);

    // RFC 6761: names in the .localhost domain resolve to loopback.
    // Honor this locally to avoid relying on external DNS servers.
    if full_host.eq_ignore_ascii_case("localhost")
        || full_host.to_ascii_lowercase().ends_with(".localhost")
    {
        debug!(
            "username-params: mapping '{}' to 127.0.0.1 due to .localhost",
            full_host
        );
        return UpstreamAddr::from_host_str_and_port("127.0.0.1", port);
    }

    debug!(
        "username-params: final next-hop host='{}' port={}",
        full_host, port
    );
    UpstreamAddr::from_host_str_and_port(full_host, port)
}

#[cfg(test)]
mod tests {
    use super::*;
    use g3_types::net::Host;

    fn cfg_with_keys(keys: &[&str]) -> UsernameParamsToEscaperConfig {
        let mut c = UsernameParamsToEscaperConfig::new(None);
        c.keys_for_host = keys.iter().map(|s| s.to_string()).collect();
        c.require_hierarchy = true;
        c.reject_unknown_keys = true;
        c.floating_keys = Vec::new();
        c.http_port = 10000;
        c.socks5_port = 10001;
        c
    }

    #[test]
    fn auth_base_works() {
        assert_eq!(ParsedUsernameParams::auth_base("user"), "user");
        assert_eq!(ParsedUsernameParams::auth_base("user+label1=foo"), "user");
        assert_eq!(ParsedUsernameParams::auth_base("u+key=v+z=t"), "u");
    }

    #[test]
    fn parse_valid_and_duplicate() {
        let p = ParsedUsernameParams::parse("user+label1=foo+label2=bar").unwrap();
        assert_eq!(p.base, "user");
        assert_eq!(p.params.get("label1").unwrap(), "foo");
        assert_eq!(p.params.get("label2").unwrap(), "bar");

        // duplicate key should error
        assert!(ParsedUsernameParams::parse("user+label1=foo+label1=baz").is_err());
    }

    // removed: we no longer override when no params are present

    #[test]
    fn compute_join_and_ports() {
        let cfg = cfg_with_keys(&["label1", "label2", "label3"]);
        let ups = compute_upstream_from_username(
            &cfg,
            "user+label1=foo+label2=bar",
            InboundKind::Http,
        )
        .unwrap();
        assert_eq!(ups.port(), 10000);
        match ups.host() {
            Host::Domain(d) => assert_eq!(d.as_ref(), "foo-bar"),
            _ => panic!("expected domain host"),
        }

        let ups2 = compute_upstream_from_username(
            &cfg,
            "user+label1=foo+label2=bar",
            InboundKind::Socks5,
        )
        .unwrap();
        assert_eq!(ups2.port(), 10001);
    }

    #[test]
    fn compute_unknown_and_hierarchy() {
        let mut cfg = cfg_with_keys(&["label1", "label2"]);
        // unknown keys rejected
        assert!(compute_upstream_from_username(&cfg, "user+x=y", InboundKind::Http).is_err());

        // allow unknown keys when disabled; but no known keys -> no override should be applied by callers
        // we still treat direct call with only unknown keys as an error now
        cfg.reject_unknown_keys = false;
        assert!(compute_upstream_from_username(&cfg, "user+x=y", InboundKind::Http).is_err());

        // hierarchy enforced: label 2 without label1 must error
        cfg.reject_unknown_keys = true;
        cfg.require_hierarchy = true;
        assert!(compute_upstream_from_username(&cfg, "user+label2=b", InboundKind::Http).is_err());

        // disable hierarchy and allow trailing keys
        cfg.require_hierarchy = false;
        let ups2 = compute_upstream_from_username(&cfg, "user+label2=b", InboundKind::Http)
            .unwrap();
        match ups2.host() {
            Host::Domain(d) => assert_eq!(d.as_ref(), "b"),
            _ => panic!("expected domain host"),
        }
    }

    #[test]
    fn compute_with_suffix() {
        let mut cfg = cfg_with_keys(&["label1"]);
        cfg.domain_suffix = Some(".svc.local".to_string());
        let ups =
            compute_upstream_from_username(&cfg, "user+label1=foo", InboundKind::Http).unwrap();
        match ups.host() {
            Host::Domain(d) => assert_eq!(d.as_ref(), "foo.svc.local"),
            _ => panic!("expected domain host"),
        }
    }

    #[test]
    fn compute_with_floating_optional() {
        // label order: label1, label2, label3, label4, opt; opt is floating (independent)
        let mut cfg = cfg_with_keys(&["label1", "label2", "label3", "label4", "opt"]);
        cfg.floating_keys = vec!["opt".to_string()];

        // opt only
        let ups = compute_upstream_from_username(&cfg, "user+opt=o123", InboundKind::Http)
            .unwrap();
        match ups.host() {
            Host::Domain(d) => assert_eq!(d.as_ref(), "o123"),
            _ => panic!("expected domain host"),
        }

        // label1 + opt
        let ups = compute_upstream_from_username(
            &cfg,
            "user+label1=foo+opt=o123",
            InboundKind::Http,
        )
        .unwrap();
        match ups.host() {
            Host::Domain(d) => assert_eq!(d.as_ref(), "foo-o123"),
            _ => panic!("expected domain host"),
        }

        // full hierarchy + opt
        let ups = compute_upstream_from_username(
            &cfg,
            "user+label1=foo+label2=bar+label3=baz+label4=qux+opt=o123",
            InboundKind::Http,
        )
        .unwrap();
        match ups.host() {
            Host::Domain(d) => assert_eq!(d.as_ref(), "foo-bar-baz-qux-o123"),
            _ => panic!("expected domain host"),
        }

        // label2 without label1 (still invalid), even if opt present
        assert!(compute_upstream_from_username(
            &cfg,
            "user+label2=bar+opt=o123",
            InboundKind::Http,
        )
        .is_err());
    }
}
