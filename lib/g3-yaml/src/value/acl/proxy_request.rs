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

use g3_types::acl::{AclProxyRequestRule, ActionContract};

use super::AclRuleYamlParser;

impl<Action: ActionContract> AclRuleYamlParser<Action> for AclProxyRequestRule<Action> {
    #[inline]
    fn get_default_found_action(&self) -> Action {
        Action::default_permit()
    }

    #[inline]
    fn set_missed_action(&mut self, action: Action) {
        self.set_missed_action(action);
    }

    fn add_rule_for_action(&mut self, action: Action, value: &Yaml) -> anyhow::Result<()> {
        let t = crate::value::as_proxy_request_type(value)?;
        self.add_request_type(t, action);
        Ok(())
    }
}

pub fn as_proxy_request_rule<Action: ActionContract>(
    value: &Yaml,
) -> anyhow::Result<AclProxyRequestRule<Action>> {
    let mut builder = AclProxyRequestRule::new(Action::default_forbid());
    builder.parse(value)?;
    Ok(builder)
}
