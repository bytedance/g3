/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use yaml_rust::Yaml;

use g3_dpi::ProtocolInspectAction;
use g3_types::acl::AclExactHostRule;

use super::InspectRuleYamlParser;

impl InspectRuleYamlParser for AclExactHostRule<ProtocolInspectAction> {
    fn add_rule_for_action(
        &mut self,
        action: ProtocolInspectAction,
        value: &Yaml,
    ) -> anyhow::Result<()> {
        let host = crate::value::as_host(value)?;
        self.add_host(host, action);
        Ok(())
    }
}

pub(super) fn as_exact_host_rule(
    value: &Yaml,
) -> anyhow::Result<AclExactHostRule<ProtocolInspectAction>> {
    let mut builder = AclExactHostRule::new(ProtocolInspectAction::Intercept);
    builder.parse(value)?;
    Ok(builder)
}
