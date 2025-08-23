/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use yaml_rust::Yaml;

use g3_types::acl::{AclAction, AclNetworkRuleBuilder};

use super::AclRuleYamlParser;

impl AclRuleYamlParser for AclNetworkRuleBuilder {
    #[inline]
    fn get_default_found_action(&self) -> AclAction {
        AclAction::Permit
    }

    #[inline]
    fn set_missed_action(&mut self, action: AclAction) {
        self.set_missed_action(action);
    }

    fn add_rule_for_action(&mut self, action: AclAction, value: &Yaml) -> anyhow::Result<()> {
        let net = crate::value::as_ip_network(value)?;
        self.add_network(net, action);
        Ok(())
    }
}

pub(crate) fn as_dst_subnet_rule_builder(value: &Yaml) -> anyhow::Result<AclNetworkRuleBuilder> {
    let mut builder = AclNetworkRuleBuilder::new_egress(AclAction::Forbid);
    builder.parse(value)?;
    Ok(builder)
}

pub fn as_egress_network_rule_builder(value: &Yaml) -> anyhow::Result<AclNetworkRuleBuilder> {
    let mut builder = AclNetworkRuleBuilder::new_egress(AclAction::Forbid);
    builder.parse(value)?;
    Ok(builder)
}

pub fn as_ingress_network_rule_builder(value: &Yaml) -> anyhow::Result<AclNetworkRuleBuilder> {
    let mut builder = AclNetworkRuleBuilder::new_ingress(AclAction::Forbid);
    builder.parse(value)?;
    Ok(builder)
}
