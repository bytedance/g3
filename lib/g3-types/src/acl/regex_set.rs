/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::{BTreeMap, HashMap};

use regex::{Regex, RegexSet};

use super::{AclAction, ActionContract, OrderedActionContract};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RegexSetBuilder<Action = AclAction> {
    inner: HashMap<String, Action>,
}

impl<Action: ActionContract> Default for RegexSetBuilder<Action> {
    fn default() -> Self {
        RegexSetBuilder {
            inner: HashMap::new(),
        }
    }
}

impl<Action: ActionContract> RegexSetBuilder<Action> {
    pub(super) fn add_regex(&mut self, regex: &Regex, action: Action) {
        self.inner.insert(regex.as_str().to_string(), action);
    }
}

impl<Action: OrderedActionContract> RegexSetBuilder<Action> {
    pub(super) fn build(&self) -> RegexSetMatch<Action> {
        let mut action_map: BTreeMap<Action, Vec<&str>> = BTreeMap::new();
        for (r, action) in &self.inner {
            action_map.entry(*action).or_default().push(r.as_str());
        }

        let action_map = action_map
            .into_iter()
            .map(|(action, v)| (action, RegexSet::new(v).unwrap()))
            .collect();

        RegexSetMatch { inner: action_map }
    }
}

pub(super) struct RegexSetMatch<Action = AclAction> {
    inner: BTreeMap<Action, RegexSet>,
}

impl<Action: ActionContract> RegexSetMatch<Action> {
    pub fn check(&self, text: &str) -> Option<Action> {
        for (action, rs) in &self.inner {
            if rs.is_match(text) {
                return Some(*action);
            }
        }
        None
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AclRegexSetRuleBuilder<Action = AclAction> {
    inner: RegexSetBuilder<Action>,
    missed_action: Action,
}

impl<Action: ActionContract> AclRegexSetRuleBuilder<Action> {
    pub fn new(missed_action: Action) -> Self {
        AclRegexSetRuleBuilder {
            inner: RegexSetBuilder::default(),
            missed_action,
        }
    }

    #[inline]
    pub fn add_regex(&mut self, regex: &Regex, action: Action) {
        self.inner.add_regex(regex, action);
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
        AclRegexSetRule {
            action_map: self.inner.build(),
            missed_action: self.missed_action,
        }
    }
}

pub struct AclRegexSetRule<Action = AclAction> {
    action_map: RegexSetMatch<Action>,
    missed_action: Action,
}

impl<Action: ActionContract> AclRegexSetRule<Action> {
    pub fn check(&self, text: &str) -> (bool, Action) {
        if let Some(action) = self.action_map.check(text) {
            return (true, action);
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
