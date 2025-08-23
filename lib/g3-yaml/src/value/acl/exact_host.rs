/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use yaml_rust::Yaml;

use g3_types::acl::{AclAction, AclExactHostRule};

use super::AclRuleYamlParser;

impl AclRuleYamlParser for AclExactHostRule {
    #[inline]
    fn get_default_found_action(&self) -> AclAction {
        AclAction::Permit
    }

    #[inline]
    fn set_missed_action(&mut self, action: AclAction) {
        self.set_missed_action(action);
    }

    fn add_rule_for_action(&mut self, action: AclAction, value: &Yaml) -> anyhow::Result<()> {
        let host = crate::value::as_host(value)?;
        self.add_host(host, action);
        Ok(())
    }
}

pub(crate) fn as_exact_host_rule(value: &Yaml) -> anyhow::Result<AclExactHostRule> {
    let mut builder = AclExactHostRule::new(AclAction::Forbid);
    builder.parse(value)?;
    Ok(builder)
}
