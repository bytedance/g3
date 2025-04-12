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

use anyhow::anyhow;
use yaml_rust::Yaml;

use g3_types::acl::{AclAction, AclRegexSetRuleBuilder};

use super::AclRuleYamlParser;

impl AclRuleYamlParser for AclRegexSetRuleBuilder {
    #[inline]
    fn get_default_found_action(&self) -> AclAction {
        AclAction::Permit
    }

    #[inline]
    fn set_missed_action(&mut self, action: AclAction) {
        self.set_missed_action(action);
    }

    fn add_rule_for_action(&mut self, action: AclAction, value: &Yaml) -> anyhow::Result<()> {
        match value {
            Yaml::String(_) => {
                let regex = crate::value::as_regex(value)?;
                self.add_regex(&regex, action);
                Ok(())
            }
            _ => Err(anyhow!("invalid value type")),
        }
    }
}

pub fn as_regex_set_rule_builder(value: &Yaml) -> anyhow::Result<AclRegexSetRuleBuilder> {
    let mut builder = AclRegexSetRuleBuilder::new(AclAction::Forbid);
    builder.parse(value)?;
    Ok(builder)
}
