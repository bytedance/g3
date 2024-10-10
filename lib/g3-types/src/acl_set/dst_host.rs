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

use crate::acl::{
    AclAction, AclChildDomainRule, AclChildDomainRuleBuilder, AclExactHostRule, AclNetworkRule,
    AclNetworkRuleBuilder, AclRegexSetRule, AclRegexSetRuleBuilder, ActionContract,
};
use crate::net::Host;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AclDstHostRuleSetBuilder<Action = AclAction> {
    pub exact: Option<AclExactHostRule<Action>>,
    pub child: Option<AclChildDomainRuleBuilder<Action>>,
    pub regex: Option<AclRegexSetRuleBuilder<Action>>,
    pub subnet: Option<AclNetworkRuleBuilder<Action>>,
    pub missing_action: Option<Action>,
}

impl<Action> Default for AclDstHostRuleSetBuilder<Action> {
    fn default() -> Self {
        Self {
            exact: None,
            child: None,
            regex: None,
            subnet: None,
            missing_action: None,
        }
    }
}

impl<Action: ActionContract> AclDstHostRuleSetBuilder<Action> {
    pub fn build(&self) -> AclDstHostRuleSet<Action> {
        let mut missed_action = self.missing_action.unwrap_or_else(Action::default_permit);

        let exact_rule = self.exact.as_ref().map(|rule| {
            missed_action = rule.missed_action().max(missed_action);
            rule.clone()
        });

        let child_rule = self.child.as_ref().map(|builder| {
            missed_action = builder.missed_action().max(missed_action);
            builder.build()
        });

        let regex_rule = self.regex.as_ref().map(|builder| {
            missed_action = builder.missed_action().max(missed_action);
            builder.build()
        });

        let subnet_rule = self.subnet.as_ref().map(|builder| {
            missed_action = builder.missed_action().max(missed_action);
            builder.build()
        });

        AclDstHostRuleSet {
            exact: exact_rule,
            child: child_rule,
            regex: regex_rule,
            subnet: subnet_rule,
            missed_action,
        }
    }
}

#[derive(Clone)]
pub struct AclDstHostRuleSet<Action: ActionContract = AclAction> {
    exact: Option<AclExactHostRule<Action>>,
    child: Option<AclChildDomainRule<Action>>,
    regex: Option<AclRegexSetRule<Action>>,
    subnet: Option<AclNetworkRule<Action>>,
    missed_action: Action,
}

impl<Action: ActionContract> AclDstHostRuleSet<Action> {
    pub fn builder() -> AclDstHostRuleSetBuilder<Action> {
        AclDstHostRuleSetBuilder::default()
    }

    pub fn builder_with_missing_action(action: Action) -> AclDstHostRuleSetBuilder<Action> {
        AclDstHostRuleSetBuilder {
            missing_action: Some(action),
            ..AclDstHostRuleSetBuilder::default()
        }
    }

    pub fn missing_action(&self) -> Action {
        self.missed_action
    }

    pub fn check(&self, upstream: &Host) -> (bool, Action) {
        match upstream {
            Host::Ip(ip) => {
                if let Some(rule) = &self.exact {
                    let (found, action) = rule.check_ip(ip);
                    if found {
                        return (true, action);
                    }
                }

                if let Some(rule) = &self.subnet {
                    let (found, action) = rule.check(*ip);
                    if found {
                        return (true, action);
                    }
                }
            }
            Host::Domain(domain) => {
                if let Some(rule) = &self.exact {
                    let (found, action) = rule.check_domain(domain);
                    if found {
                        return (true, action);
                    }
                }

                if let Some(rule) = &self.child {
                    let (found, action) = rule.check(domain);
                    if found {
                        return (true, action);
                    }
                }

                if let Some(rule) = &self.regex {
                    let (found, action) = rule.check(domain);
                    if found {
                        return (true, action);
                    }
                }
            }
        }

        (false, self.missed_action)
    }
}
