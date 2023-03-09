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

use std::net::IpAddr;

use crate::net::Host;

use super::{AclAHashRule, AclAction};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AclExactHostRule {
    missed_action: AclAction,
    domain: AclAHashRule<String>,
    ip: AclAHashRule<IpAddr>,
}

impl AclExactHostRule {
    #[inline]
    pub fn new(missed_action: AclAction) -> Self {
        AclExactHostRule {
            missed_action,
            domain: AclAHashRule::new(missed_action),
            ip: AclAHashRule::new(missed_action),
        }
    }

    #[inline]
    pub fn add_domain(&mut self, domain: String, action: AclAction) {
        self.domain.add_node(domain, action);
    }

    #[inline]
    pub fn add_ip(&mut self, ip: IpAddr, action: AclAction) {
        self.ip.add_node(ip, action);
    }

    pub fn add_host(&mut self, host: Host, action: AclAction) {
        match host {
            Host::Ip(ip) => self.add_ip(ip, action),
            Host::Domain(domain) => self.add_domain(domain, action),
        }
    }

    #[inline]
    pub fn set_missed_action(&mut self, action: AclAction) {
        self.missed_action = action;
        self.domain.set_missed_action(action);
        self.ip.set_missed_action(action);
    }

    #[inline]
    pub fn missed_action(&self) -> AclAction {
        self.missed_action
    }

    #[inline]
    pub fn check_domain(&self, domain: &str) -> (bool, AclAction) {
        self.domain.check(domain)
    }

    #[inline]
    pub fn check_ip(&self, ip: &IpAddr) -> (bool, AclAction) {
        self.ip.check(ip)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;
    use std::str::FromStr;

    #[test]
    fn check() {
        let mut rule = AclExactHostRule::new(AclAction::Forbid);
        rule.add_domain("www.example.com".to_string(), AclAction::Permit);
        rule.add_ip(
            IpAddr::from_str("192.168.1.1").unwrap(),
            AclAction::PermitAndLog,
        );

        assert_eq!(
            rule.check_domain("www.example.com"),
            (true, AclAction::Permit)
        );
        assert_eq!(
            rule.check_domain("www.example.net"),
            (false, AclAction::Forbid)
        );
        assert_eq!(
            rule.check_ip(&IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))),
            (true, AclAction::PermitAndLog)
        );
        assert_eq!(
            rule.check_ip(&IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2))),
            (false, AclAction::Forbid)
        );

        rule.set_missed_action(AclAction::ForbidAndLog);
        assert_eq!(
            rule.check_domain("www.example.net"),
            (false, AclAction::ForbidAndLog)
        );
    }
}
