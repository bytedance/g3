/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_types::acl::{AclAction, AclRegexDomainRuleBuilder};

use super::AclRuleYamlParser;

impl AclRuleYamlParser for AclRegexDomainRuleBuilder {
    #[inline]
    fn get_default_found_action(&self) -> AclAction {
        AclAction::Permit
    }

    #[inline]
    fn set_missed_action(&mut self, action: AclAction) {
        self.set_missed_action(action);
    }

    fn add_rule_for_action(&mut self, action: AclAction, value: &Yaml) -> anyhow::Result<()> {
        match value {
            Yaml::Hash(map) => {
                let parent_v = crate::hash::get_required(map, "parent")?;
                let parent_domain = crate::value::as_domain(parent_v)
                    .context("invalid domain string value for key 'parent'")?;
                let regex_v = crate::hash::get_required(map, "regex")?;
                match regex_v {
                    Yaml::Array(seq) => {
                        for (i, v) in seq.iter().enumerate() {
                            let regex = crate::value::as_regex(v)
                                .context(format!("invalid regex string value for 'regex/{i}'"))?;
                            self.add_prefix_regex(&parent_domain, &regex, action);
                        }
                        Ok(())
                    }
                    Yaml::String(_) => {
                        let regex = crate::value::as_regex(regex_v)
                            .context("invalid regex string value for key 'regex'")?;
                        self.add_prefix_regex(&parent_domain, &regex, action);
                        Ok(())
                    }
                    _ => Err(anyhow!("invalid value type for key 'regex'")),
                }
            }
            Yaml::String(_) => {
                let regex = crate::value::as_regex(value)?;
                self.add_full_regex(&regex, action);
                Ok(())
            }
            _ => Err(anyhow!("invalid value type")),
        }
    }
}

pub(crate) fn as_regex_domain_rule_builder(
    value: &Yaml,
) -> anyhow::Result<AclRegexDomainRuleBuilder> {
    let mut builder = AclRegexDomainRuleBuilder::new(AclAction::Forbid);
    builder.parse(value)?;
    Ok(builder)
}
