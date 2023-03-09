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

use std::ops::RangeInclusive;

use super::{AclAction, AclFxHashRule};
use crate::net::Ports;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AclExactPortRule(AclFxHashRule<u16>);

impl AclExactPortRule {
    #[inline]
    pub fn new(missed_action: AclAction) -> Self {
        AclExactPortRule(AclFxHashRule::new(missed_action))
    }

    pub fn add_port_range(&mut self, port_range: RangeInclusive<u16>, action: AclAction) {
        for port in port_range {
            self.0.add_node(port, action);
        }
    }

    pub fn add_ports(&mut self, ports: Ports, action: AclAction) {
        for port in ports {
            self.0.add_node(port, action);
        }
    }

    #[inline]
    pub fn add_port(&mut self, port: u16, action: AclAction) {
        self.0.add_node(port, action);
    }

    #[inline]
    pub fn set_missed_action(&mut self, action: AclAction) {
        self.0.set_missed_action(action);
    }

    #[inline]
    pub fn check_port(&self, port: &u16) -> (bool, AclAction) {
        self.0.check(port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn check() {
        let mut rule = AclExactPortRule::new(AclAction::Forbid);
        rule.add_port_range(10000..=10100, AclAction::Permit);
        rule.add_port(80, AclAction::Permit);

        assert_eq!(rule.check_port(&80), (true, AclAction::Permit));
        assert_eq!(rule.check_port(&10010), (true, AclAction::Permit));
        assert_eq!(rule.check_port(&11000), (false, AclAction::Forbid));

        rule.set_missed_action(AclAction::ForbidAndLog);
        assert_eq!(rule.check_port(&11000), (false, AclAction::ForbidAndLog));
    }
}
