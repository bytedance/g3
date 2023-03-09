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

use std::collections::HashMap;

use regex::{Regex, RegexSet};

use super::AclAction;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AclRegexSetRuleBuilder {
    inner: HashMap<String, AclAction>,
    missed_action: AclAction,
}

impl Default for AclRegexSetRuleBuilder {
    fn default() -> Self {
        Self::new(AclAction::Forbid)
    }
}

impl AclRegexSetRuleBuilder {
    pub fn new(missed_action: AclAction) -> Self {
        AclRegexSetRuleBuilder {
            inner: HashMap::new(),
            missed_action,
        }
    }

    #[inline]
    pub fn add_regex(&mut self, regex: &Regex, action: AclAction) {
        self.inner.insert(regex.as_str().to_string(), action);
    }

    #[inline]
    pub fn set_missed_action(&mut self, action: AclAction) {
        self.missed_action = action;
    }

    #[inline]
    pub fn missed_action(&self) -> AclAction {
        self.missed_action
    }

    pub fn build(&self) -> AclRegexSetRule {
        let mut forbid_log_v = Vec::new();
        let mut forbid_v = Vec::new();
        let mut permit_log_v = Vec::new();
        let mut permit_v = Vec::new();

        for (r, action) in &self.inner {
            match action {
                AclAction::ForbidAndLog => forbid_log_v.push(r.as_str()),
                AclAction::Forbid => forbid_v.push(r.as_str()),
                AclAction::PermitAndLog => permit_log_v.push(r.as_str()),
                AclAction::Permit => permit_v.push(r.as_str()),
            }
        }

        fn build_rs_from_vec(v: &[&str]) -> Option<RegexSet> {
            if v.is_empty() {
                None
            } else {
                Some(RegexSet::new(v).unwrap())
            }
        }

        AclRegexSetRule {
            forbid_log: build_rs_from_vec(&forbid_log_v),
            forbid: build_rs_from_vec(&forbid_v),
            permit_log: build_rs_from_vec(&permit_log_v),
            permit: build_rs_from_vec(&permit_v),
            missed_action: self.missed_action,
        }
    }
}

pub struct AclRegexSetRule {
    forbid_log: Option<RegexSet>,
    forbid: Option<RegexSet>,
    permit_log: Option<RegexSet>,
    permit: Option<RegexSet>,
    missed_action: AclAction,
}

impl AclRegexSetRule {
    pub fn check(&self, text: &str) -> (bool, AclAction) {
        if let Some(rs) = &self.forbid_log {
            if rs.is_match(text) {
                return (true, AclAction::ForbidAndLog);
            }
        }

        if let Some(rs) = &self.forbid {
            if rs.is_match(text) {
                return (true, AclAction::Forbid);
            }
        }

        if let Some(rs) = &self.permit_log {
            if rs.is_match(text) {
                return (true, AclAction::PermitAndLog);
            }
        }

        if let Some(rs) = &self.permit {
            if rs.is_match(text) {
                return (true, AclAction::Permit);
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
