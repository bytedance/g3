/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::ops::RangeInclusive;

use super::{AclAction, AclFxHashRule, ActionContract};
use crate::net::Ports;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AclExactPortRule<Action = AclAction>(AclFxHashRule<u16, Action>);

impl<Action: ActionContract> AclExactPortRule<Action> {
    #[inline]
    pub fn new(missed_action: Action) -> Self {
        AclExactPortRule(AclFxHashRule::new(missed_action))
    }

    pub fn add_port_range(&mut self, port_range: RangeInclusive<u16>, action: Action) {
        for port in port_range {
            self.0.add_node(port, action);
        }
    }

    pub fn add_ports(&mut self, ports: Ports, action: Action) {
        for port in ports {
            self.0.add_node(port, action);
        }
    }

    #[inline]
    pub fn add_port(&mut self, port: u16, action: Action) {
        self.0.add_node(port, action);
    }

    #[inline]
    pub fn set_missed_action(&mut self, action: Action) {
        self.0.set_missed_action(action);
    }

    #[inline]
    pub fn check_port(&self, port: &u16) -> (bool, Action) {
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
