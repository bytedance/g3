/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use serde_json::Value;

use g3_types::acl::{AclAction, AclNetworkRuleBuilder};

use super::AclRuleJsonParser;

impl AclRuleJsonParser for AclNetworkRuleBuilder {
    #[inline]
    fn get_default_found_action(&self) -> AclAction {
        AclAction::Permit
    }

    #[inline]
    fn set_missed_action(&mut self, _action: AclAction) {
        self.set_missed_action(_action);
    }

    fn add_rule_for_action(&mut self, action: AclAction, value: &Value) -> anyhow::Result<()> {
        let net = crate::value::as_ip_network(value)?;
        self.add_network(net, action);
        Ok(())
    }
}

pub(crate) fn as_dst_subnet_network_rule_builder(
    value: &Value,
) -> anyhow::Result<AclNetworkRuleBuilder> {
    let mut builder = AclNetworkRuleBuilder::new_egress(AclAction::Forbid);
    builder.parse(value)?;
    Ok(builder)
}

pub fn as_egress_network_rule_builder(value: &Value) -> anyhow::Result<AclNetworkRuleBuilder> {
    let mut builder = AclNetworkRuleBuilder::new_egress(AclAction::Forbid);
    builder.parse(value)?;
    Ok(builder)
}

pub fn as_ingress_network_rule_builder(value: &Value) -> anyhow::Result<AclNetworkRuleBuilder> {
    let mut builder = AclNetworkRuleBuilder::new_ingress(AclAction::Forbid);
    builder.parse(value)?;
    Ok(builder)
}
