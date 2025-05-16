/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use yaml_rust::Yaml;

use g3_dpi::ProtocolInspectAction;
use g3_types::acl::AclChildDomainRuleBuilder;

use super::InspectRuleYamlParser;

impl InspectRuleYamlParser for AclChildDomainRuleBuilder<ProtocolInspectAction> {
    fn add_rule_for_action(
        &mut self,
        action: ProtocolInspectAction,
        value: &Yaml,
    ) -> anyhow::Result<()> {
        match value {
            Yaml::String(_) => {
                let host = crate::value::as_domain(value)?;
                self.add_node(&host, action);
                Ok(())
            }
            _ => Err(anyhow!("invalid value type")),
        }
    }
}

pub(super) fn as_child_domain_rule_builder(
    value: &Yaml,
) -> anyhow::Result<AclChildDomainRuleBuilder<ProtocolInspectAction>> {
    let mut builder = AclChildDomainRuleBuilder::new(ProtocolInspectAction::Intercept);
    builder.parse(value)?;
    Ok(builder)
}
