/*
 * Copyright 2024 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
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
