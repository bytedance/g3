/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use serde_json::Value;

use g3_types::acl::{AclAction, AclProxyRequestRule};

use super::AclRuleJsonParser;

impl AclRuleJsonParser for AclProxyRequestRule {
    #[inline]
    fn get_default_found_action(&self) -> AclAction {
        AclAction::Permit
    }

    #[inline]
    fn set_missed_action(&mut self, _action: AclAction) {
        self.set_missed_action(_action);
    }

    fn add_rule_for_action(&mut self, action: AclAction, value: &Value) -> anyhow::Result<()> {
        let t = crate::value::as_proxy_request_type(value)?;
        self.add_request_type(t, action);
        Ok(())
    }
}

pub fn as_proxy_request_rule(value: &Value) -> anyhow::Result<AclProxyRequestRule> {
    let mut builder = AclProxyRequestRule::new(AclAction::Forbid);
    builder.parse(value)?;
    Ok(builder)
}
