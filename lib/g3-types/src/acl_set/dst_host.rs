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
    AclNetworkRuleBuilder, AclRegexSetRule, AclRegexSetRuleBuilder,
};
use crate::net::Host;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AclDstHostRuleSetBuilder {
    pub exact: Option<AclExactHostRule>,
    pub child: Option<AclChildDomainRuleBuilder>,
    pub regex: Option<AclRegexSetRuleBuilder>,
    pub subnet: Option<AclNetworkRuleBuilder>,
}

impl AclDstHostRuleSetBuilder {
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

pub struct AclDstHostRuleSet {
    exact: Option<AclExactHostRule>,
    child: Option<AclChildDomainRule>,
    regex: Option<AclRegexSetRule>,
    subnet: Option<AclNetworkRule>,
    missed_action: AclAction,
}

impl AclDstHostRuleSet {
    pub fn check(&self, upstream: &Host) -> (bool, AclAction) {
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
