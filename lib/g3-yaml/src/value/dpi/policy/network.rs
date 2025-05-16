/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use yaml_rust::Yaml;

use g3_dpi::ProtocolInspectAction;
use g3_types::acl::AclNetworkRuleBuilder;

use super::InspectRuleYamlParser;

impl InspectRuleYamlParser for AclNetworkRuleBuilder<ProtocolInspectAction> {
    fn add_rule_for_action(
        &mut self,
        action: ProtocolInspectAction,
        value: &Yaml,
    ) -> anyhow::Result<()> {
        let net = crate::value::as_ip_network(value)?;
        self.add_network(net, action);
        Ok(())
    }
}

pub(super) fn as_dst_subnet_rule_builder(
    value: &Yaml,
) -> anyhow::Result<AclNetworkRuleBuilder<ProtocolInspectAction>> {
    let mut builder = AclNetworkRuleBuilder::new(ProtocolInspectAction::Intercept);
    builder.parse(value)?;
    Ok(builder)
}
