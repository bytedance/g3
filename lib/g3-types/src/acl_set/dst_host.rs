/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use crate::acl::{
    AclAction, AclChildDomainRule, AclChildDomainRuleBuilder, AclExactHostRule, AclNetworkRule,
    AclNetworkRuleBuilder, AclRegexDomainRule, AclRegexDomainRuleBuilder, ActionContract,
    OrderedActionContract,
};
use crate::net::Host;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AclDstHostRuleSetBuilder<Action = AclAction> {
    pub exact: Option<AclExactHostRule<Action>>,
    pub child: Option<AclChildDomainRuleBuilder<Action>>,
    pub regex: Option<AclRegexDomainRuleBuilder<Action>>,
    pub subnet: Option<AclNetworkRuleBuilder<Action>>,
}

impl<Action> Default for AclDstHostRuleSetBuilder<Action> {
    fn default() -> Self {
        AclDstHostRuleSetBuilder {
            exact: None,
            child: None,
            regex: None,
            subnet: None,
        }
    }
}

impl<Action: OrderedActionContract> AclDstHostRuleSetBuilder<Action> {
    pub fn build_with_missed_action(&self, missed_action: Action) -> AclDstHostRuleSet<Action> {
        AclDstHostRuleSet {
            exact: self.exact.clone(),
            child: self.child.as_ref().map(|b| b.build()),
            regex: self.regex.as_ref().map(|b| b.build()),
            subnet: self.subnet.as_ref().map(|b| b.build()),
            missed_action,
        }
    }
}

impl AclDstHostRuleSetBuilder<AclAction> {
    pub fn build(&self) -> AclDstHostRuleSet {
        let mut missed_action = AclAction::Permit;

        let exact_rule = self.exact.as_ref().map(|rule| {
            missed_action = missed_action.restrict(rule.missed_action());
            rule.clone()
        });

        let child_rule = self.child.as_ref().map(|builder| {
            missed_action = missed_action.restrict(builder.missed_action());
            builder.build()
        });

        let regex_rule = self.regex.as_ref().map(|builder| {
            missed_action = missed_action.restrict(builder.missed_action());
            builder.build()
        });

        let subnet_rule = self.subnet.as_ref().map(|builder| {
            missed_action = missed_action.restrict(builder.missed_action());
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

pub struct AclDstHostRuleSet<Action = AclAction> {
    exact: Option<AclExactHostRule<Action>>,
    child: Option<AclChildDomainRule<Action>>,
    regex: Option<AclRegexDomainRule<Action>>,
    subnet: Option<AclNetworkRule<Action>>,
    missed_action: Action,
}

impl<Action: ActionContract> AclDstHostRuleSet<Action> {
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
