/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use serde_json::Value;

use g3_types::acl_set::AclDstHostRuleSetBuilder;

pub fn as_dst_host_rule_set_builder(value: &Value) -> anyhow::Result<AclDstHostRuleSetBuilder> {
    if let Value::Object(map) = value {
        let mut builder = AclDstHostRuleSetBuilder::default();
        for (k, v) in map {
            match crate::key::normalize(k).as_str() {
                "exact_match" | "exact" => {
                    let exact_rule = crate::value::acl::as_exact_host_rule(v)
                        .context(format!("invalid exact host acl rule value for key {k}"))?;
                    builder.exact = Some(exact_rule);
                }
                "child_match" | "child" => {
                    let child_builder = crate::value::acl::as_child_domain_rule_builder(v)
                        .context(format!("invalid child domain acl rule value for key {k}"))?;
                    builder.child = Some(child_builder);
                }
                "regex_match" | "regex" => {
                    let regex_builder = crate::value::acl::as_regex_domain_rule_builder(v)
                        .context(format!("invalid regex domain rule value for key {k}"))?;
                    builder.regex = Some(regex_builder);
                }
                "subnet_match" | "subnet" => {
                    let subnet_builder =
                        crate::value::acl::as_dst_subnet_network_rule_builder(v)
                            .context(format!("invalid subnet acl rule value for key {k}"))?;
                    builder.subnet = Some(subnet_builder);
                }
                _ => return Err(anyhow!("invalid key {k}")),
            }
        }
        Ok(builder)
    } else {
        Err(anyhow!("invalid value type"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g3_types::acl::AclAction;
    use g3_types::net::Host;
    use serde_json::json;
    use std::str::FromStr;

    #[test]
    fn t_dst_host() {
        let j = json!({
            "v": {
                "exact_match": [
                    "match1.example.net",
                    "192.168.2.1"
                ],
                "child_match": "example.org",
                "regex_match": [
                    ".*2[.]example[.]net$"
                ],
                "subnet_match": {
                    "allow": [
                        "192.168.3.0/24",
                        "172.16.0.0/16"
                    ],
                    "forbid_log": "127.0.0.1"
                }
            }
        });
        let builder = as_dst_host_rule_set_builder(&j["v"]).unwrap();
        let rule = builder.build();

        assert_eq!(
            rule.check(&Host::from_str("match1.example.net").unwrap()),
            (true, AclAction::Permit)
        );
        assert_eq!(
            rule.check(&Host::from_str("all.example.org").unwrap()),
            (true, AclAction::Permit)
        );
        assert_eq!(
            rule.check(&Host::from_str("found2.example.net").unwrap()),
            (true, AclAction::Permit)
        );
        assert_eq!(
            rule.check(&Host::from_str("not-found3.example.net").unwrap()),
            (false, AclAction::Forbid)
        );
        assert_eq!(
            rule.check(&Host::from_str("192.168.2.1").unwrap()),
            (true, AclAction::Permit)
        );
        assert_eq!(
            rule.check(&Host::from_str("192.168.3.1").unwrap()),
            (true, AclAction::Permit)
        );
        assert_eq!(
            rule.check(&Host::from_str("192.168.4.1").unwrap()),
            (false, AclAction::Forbid)
        );
        assert_eq!(
            rule.check(&Host::from_str("127.0.0.1").unwrap()),
            (true, AclAction::ForbidAndLog)
        );
    }
}
