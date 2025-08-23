/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use serde_json::Value;

use g3_types::acl::{AclAction, AclExactHostRule};

use super::AclRuleJsonParser;

impl AclRuleJsonParser for AclExactHostRule {
    #[inline]
    fn get_default_found_action(&self) -> AclAction {
        AclAction::Permit
    }

    #[inline]
    fn set_missed_action(&mut self, _action: AclAction) {
        self.set_missed_action(_action);
    }

    fn add_rule_for_action(&mut self, action: AclAction, value: &Value) -> anyhow::Result<()> {
        let host = crate::value::as_host(value)?;
        self.add_host(host, action);
        Ok(())
    }
}

pub(crate) fn as_exact_host_rule(value: &Value) -> anyhow::Result<AclExactHostRule> {
    let mut builder = AclExactHostRule::new(AclAction::Forbid);
    builder.parse(value)?;
    Ok(builder)
}
