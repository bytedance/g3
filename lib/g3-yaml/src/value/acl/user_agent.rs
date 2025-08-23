/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::Context;
use yaml_rust::Yaml;

use g3_types::acl::{AclAction, AclUserAgentRule};

use super::AclRuleYamlParser;

impl AclRuleYamlParser for AclUserAgentRule {
    #[inline]
    fn get_default_found_action(&self) -> AclAction {
        AclAction::Forbid
    }

    #[inline]
    fn set_missed_action(&mut self, action: AclAction) {
        self.set_missed_action(action);
    }

    fn add_rule_for_action(&mut self, action: AclAction, value: &Yaml) -> anyhow::Result<()> {
        let ua_name = crate::value::as_ascii(value)
            .context("user-agent name should be valid ascii string")?;
        self.add_ua_name(ua_name.as_str(), action);
        Ok(())
    }
}

pub fn as_user_agent_rule(value: &Yaml) -> anyhow::Result<AclUserAgentRule> {
    let mut builder = AclUserAgentRule::new(AclAction::Permit);
    builder.parse(value)?;
    Ok(builder)
}
