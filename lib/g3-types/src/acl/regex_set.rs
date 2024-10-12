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

use std::collections::{BTreeMap, HashMap};

use regex::{Regex, RegexSet};

use super::{AclAction, ActionContract, OrderedActionContract};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AclRegexSetRuleBuilder<Action = AclAction> {
    inner: HashMap<String, Action>,
    missed_action: Action,
}

impl<Action: ActionContract> AclRegexSetRuleBuilder<Action> {
    pub fn new(missed_action: Action) -> Self {
        AclRegexSetRuleBuilder {
            inner: HashMap::new(),
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
}

impl<Action: OrderedActionContract> AclRegexSetRuleBuilder<Action> {
    pub fn build(&self) -> AclRegexSetRule<Action> {
        let mut action_map: BTreeMap<Action, Vec<&str>> = BTreeMap::new();
        for (r, action) in &self.inner {
            action_map.entry(*action).or_default().push(r.as_str());
        }

        let action_map = action_map
            .into_iter()
            .map(|(action, v)| (action, RegexSet::new(v).unwrap()))
            .collect();

        AclRegexSetRule {
            action_map,
            missed_action: self.missed_action,
        }
    }
}

pub struct AclRegexSetRule<Action = AclAction> {
    action_map: BTreeMap<Action, RegexSet>,
    missed_action: Action,
}

impl<Action: ActionContract> AclRegexSetRule<Action> {
    pub fn check(&self, text: &str) -> (bool, Action) {
        for (action, rs) in &self.action_map {
            if rs.is_match(text) {
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

    #[test]
    fn check_order() {
        let mut builder = AclRegexSetRuleBuilder::new(AclAction::Forbid);

        let regex = Regex::new("[.]net$").unwrap();
        builder.add_regex(&regex, AclAction::Permit);

        let regex = Regex::new("example[.]net$").unwrap();
        builder.add_regex(&regex, AclAction::PermitAndLog);

        let regex = Regex::new("f[.]example[.]net$").unwrap();
        builder.add_regex(&regex, AclAction::ForbidAndLog);

        let rule = builder.build();

        assert_eq!(
            rule.check("www.example.net"),
            (true, AclAction::PermitAndLog)
        );
        assert_eq!(rule.check("a.example1.net"), (true, AclAction::Permit));
        assert_eq!(rule.check("f.example.net"), (true, AclAction::ForbidAndLog));
    }
}
