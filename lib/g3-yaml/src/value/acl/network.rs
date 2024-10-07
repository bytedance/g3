/*
 * Copyright 2023 ByteDance and/or its affiliates.
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

use g3_types::acl::{AclNetworkRuleBuilder, ActionContract};

use super::AclRuleYamlParser;

impl<Action: ActionContract> AclRuleYamlParser<Action> for AclNetworkRuleBuilder<Action> {
    #[inline]
    fn get_default_found_action(&self) -> Action {
        Action::default_permit()
    }

    #[inline]
    fn set_missed_action(&mut self, action: Action) {
        self.set_missed_action(action);
    }

    fn add_rule_for_action(&mut self, action: Action, value: &Yaml) -> anyhow::Result<()> {
        let net = crate::value::as_ip_network(value)?;
        self.add_network(net, action);
        Ok(())
    }
}

pub(crate) fn as_dst_subnet_rule_builder<Action: ActionContract>(
    value: &Yaml,
) -> anyhow::Result<AclNetworkRuleBuilder<Action>> {
    let mut builder = AclNetworkRuleBuilder::new_egress(Action::default_forbid());
    builder.parse(value)?;
    Ok(builder)
}

pub fn as_egress_network_rule_builder<Action: ActionContract>(
    value: &Yaml,
) -> anyhow::Result<AclNetworkRuleBuilder<Action>> {
    let mut builder = AclNetworkRuleBuilder::new_egress(Action::default_forbid());
    builder.parse(value)?;
    Ok(builder)
}

pub fn as_ingress_network_rule_builder<Action: ActionContract>(
    value: &Yaml,
) -> anyhow::Result<AclNetworkRuleBuilder<Action>> {
    let mut builder = AclNetworkRuleBuilder::new_ingress(Action::default_forbid());
    builder.parse(value)?;
    Ok(builder)
}
