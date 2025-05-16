/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use serde_json::Value;

use g3_types::acl::{AclAction, AclChildDomainRuleBuilder};

use super::AclRuleJsonParser;

impl AclRuleJsonParser for AclChildDomainRuleBuilder {
    #[inline]
    fn get_default_found_action(&self) -> AclAction {
        AclAction::Permit
    }

    #[inline]
    fn set_missed_action(&mut self, _action: AclAction) {
        self.set_missed_action(_action);
    }

    fn add_rule_for_action(&mut self, action: AclAction, value: &Value) -> anyhow::Result<()> {
        match value {
            Value::String(_) => {
                let domain = crate::value::as_domain(value)?;
                self.add_node(&domain, action);
                Ok(())
            }
            _ => Err(anyhow!("invalid value type")),
        }
    }
}

pub(crate) fn as_child_domain_rule_builder(
    value: &Value,
) -> anyhow::Result<AclChildDomainRuleBuilder> {
    let mut builder = AclChildDomainRuleBuilder::new(AclAction::Forbid);
    builder.parse(value)?;
    Ok(builder)
}
