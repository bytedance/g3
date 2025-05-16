/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_types::acl_set::AclDstHostRuleSetBuilder;

pub fn as_dst_host_rule_set_builder(value: &Yaml) -> anyhow::Result<AclDstHostRuleSetBuilder> {
    if let Yaml::Hash(map) = value {
        let mut builder = AclDstHostRuleSetBuilder::default();

        crate::foreach_kv(map, |k, v| match crate::key::normalize(k).as_str() {
            "exact_match" | "exact" => {
                let exact_rule = crate::value::acl::as_exact_host_rule(v)
                    .context(format!("invalid exact host acl rule value for key {k}"))?;
                builder.exact = Some(exact_rule);
                Ok(())
            }
            "child_match" | "child" => {
                let child_builder = crate::value::acl::as_child_domain_rule_builder(v)
                    .context(format!("invalid child domain acl rule value for key {k}"))?;
                builder.child = Some(child_builder);
                Ok(())
            }
            "regex_match" | "regex" => {
                let regex_builder = crate::value::acl::as_regex_domain_rule_builder(v)
                    .context(format!("invalid regex domain acl rule value for key {k}"))?;
                builder.regex = Some(regex_builder);
                Ok(())
            }
            "subnet_match" | "subnet" => {
                let subnet_builder = crate::value::acl::as_dst_subnet_rule_builder(v)
                    .context(format!("invalid subnet acl rule value for key {k}"))?;
                builder.subnet = Some(subnet_builder);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        })?;
        Ok(builder)
    } else {
        Err(anyhow!("invalid value type"))
    }
}
