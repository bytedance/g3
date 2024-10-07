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

use super::{AclAction, AclRadixTrieRule, AclRadixTrieRuleBuilder, ActionContract};
use crate::resolve::reverse_idna_domain;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AclChildDomainRuleBuilder<Action = AclAction>(AclRadixTrieRuleBuilder<String, Action>);

impl<Action: ActionContract> AclChildDomainRuleBuilder<Action> {
    #[inline]
    pub fn new(missed_action: Action) -> Self {
        AclChildDomainRuleBuilder(AclRadixTrieRuleBuilder::new(missed_action))
    }

    #[inline]
    pub fn add_node(&mut self, domain: &str, action: Action) {
        self.0.add_node(reverse_idna_domain(domain), action);
    }

    #[inline]
    pub fn set_missed_action(&mut self, action: Action) {
        self.0.set_missed_action(action);
    }

    #[inline]
    pub fn missed_action(&self) -> Action {
        self.0.missed_action()
    }

    #[inline]
    pub fn build(&self) -> AclChildDomainRule<Action> {
        AclChildDomainRule(self.0.build())
    }
}

#[derive(Clone)]
pub struct AclChildDomainRule<Action = AclAction>(AclRadixTrieRule<String, Action>);

impl<Action: ActionContract> AclChildDomainRule<Action> {
    #[inline]
    pub fn check(&self, host: &str) -> (bool, Action) {
        let s = reverse_idna_domain(host);
        self.0.check(&s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check() {
        let mut builder = AclChildDomainRuleBuilder::new(AclAction::Forbid);
        builder.add_node("foo.com", AclAction::Permit);
        let rule = builder.build();

        assert_eq!(rule.check("foo.com"), (true, AclAction::Permit));
        assert_eq!(rule.check("a.foo.com"), (true, AclAction::Permit));
        assert_eq!(rule.check("a.fooz.com"), (false, AclAction::Forbid));
        assert_eq!(rule.check("a.zfoo.com"), (false, AclAction::Forbid));
    }
}
