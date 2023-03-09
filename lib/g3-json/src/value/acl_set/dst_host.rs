/*
 * Copyright 2023 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use anyhow::{anyhow, Context};
use serde_json::Value;

use g3_types::acl_set::AclDstHostRuleSetBuilder;

pub fn as_dst_host_rule_set_builder(value: &Value) -> anyhow::Result<AclDstHostRuleSetBuilder> {
    if let Value::Object(map) = value {
        let mut builder = AclDstHostRuleSetBuilder {
            exact: None,
            child: None,
            regex: None,
            subnet: None,
        };
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
                    let regex_builder = crate::value::acl::as_regex_set_rule_builder(v)
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
