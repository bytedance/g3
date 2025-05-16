/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use serde_json::Value;

use g3_types::acl::{AclAction, AclExactPortRule};

use super::AclRuleJsonParser;

impl AclRuleJsonParser for AclExactPortRule {
    #[inline]
    fn get_default_found_action(&self) -> AclAction {
        AclAction::Permit
    }

    #[inline]
    fn set_missed_action(&mut self, _action: AclAction) {
        self.set_missed_action(_action);
    }

    fn add_rule_for_action(&mut self, action: AclAction, value: &Value) -> anyhow::Result<()> {
        let ports = crate::value::as_ports(value)?;
        self.add_ports(ports, action);
        Ok(())
    }
}

pub fn as_exact_port_rule(value: &Value) -> anyhow::Result<AclExactPortRule> {
    let mut builder = AclExactPortRule::new(AclAction::Forbid);
    builder.parse(value)?;
    Ok(builder)
}
