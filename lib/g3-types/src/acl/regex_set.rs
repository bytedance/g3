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

use regex::{Regex, RegexSet};
use rustc_hash::FxHashMap;

use super::{AclAction, ActionContract};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AclRegexSetRuleBuilder<Action = AclAction> {
    inner: FxHashMap<String, Action>,
    missed_action: Action,
}

impl<Action: ActionContract> Default for AclRegexSetRuleBuilder<Action> {
    fn default() -> Self {
        Self::new(Action::default_forbid())
    }
}

impl<Action: ActionContract> AclRegexSetRuleBuilder<Action> {
    pub fn new(missed_action: Action) -> Self {
        AclRegexSetRuleBuilder {
            inner: FxHashMap::default(),
            missed_action,
        }
    }

    #[inline]
    pub fn add_regex(&mut self, regex: &Regex, action: Action) {
        self.inner.insert(regex.as_str().to_string(), action);
    }

    #[inline]
    pub fn set_missed_action(&mut self, action: Action) {
        self.missed_action = action;
    }

    #[inline]
    pub fn missed_action(&self) -> Action {
        self.missed_action
    }

    pub fn build(&self) -> AclRegexSetRule<Action> {
        let mut set_map: FxHashMap<Action, Vec<_>> = FxHashMap::default();

        for (r, action) in &self.inner {
            set_map.entry(*action).or_default().push(r.as_str());
        }

        AclRegexSetRule {
            set_map: set_map
                .into_iter()
                .map(|(k, v)| (k, RegexSet::new(v).unwrap()))
                .collect(),
            missed_action: self.missed_action,
        }
    }
}

pub struct AclRegexSetRule<Action = AclAction> {
    set_map: FxHashMap<Action, RegexSet>,
    missed_action: Action,
}

impl<Action: ActionContract> AclRegexSetRule<Action> {
    pub fn check(&self, text: &str) -> (bool, Action) {
        for (action, set) in &self.set_map {
            if set.is_match(text) {
                return (true, *action);
            }
        }
        (false, self.missed_action)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check() {
        let mut builder = AclRegexSetRuleBuilder::new(AclAction::Forbid);

        let regex1 = Regex::new(".*[.]example[.]net$").unwrap();
        builder.add_regex(&regex1, AclAction::Permit);

        let regex2 = Regex::new("^www[.]example[.].*").unwrap();
        builder.add_regex(&regex2, AclAction::Permit);

        let rule = builder.build();

        assert_eq!(rule.check("www.example.net"), (true, AclAction::Permit));
        assert_eq!(rule.check("www.example.com"), (true, AclAction::Permit));
        assert_eq!(rule.check("abc.example.com"), (false, AclAction::Forbid));
    }

    #[test]
    fn check_partial() {
        let mut builder = AclRegexSetRuleBuilder::new(AclAction::Forbid);

        let regex = Regex::new("[.]net$").unwrap();
        builder.add_regex(&regex, AclAction::Permit);

        let rule = builder.build();

        assert_eq!(rule.check("www.example.net"), (true, AclAction::Permit));
    }
}
