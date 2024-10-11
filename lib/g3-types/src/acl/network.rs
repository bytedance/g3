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
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::sync::LazyLock;

use ip_network::IpNetwork;
use ip_network_table::IpNetworkTable;

use super::{AclAction, ActionContract};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AclNetworkRuleBuilder<Action = AclAction> {
    inner: HashMap<IpNetwork, Action>,
    missed_action: Action,
}

impl<Action: ActionContract> AclNetworkRuleBuilder<Action> {
    pub fn new_egress(missed_action: Action) -> Self {
        static DEFAULT_EGRESS_RULE: LazyLock<Vec<IpNetwork>> = LazyLock::new(|| {
            vec![
                // forbid ipv4 unspecified 0.0.0.0/32 by default
                IpNetwork::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 32).unwrap(),
                // forbid ipv4 loopback 127.0.0.0/8 by default
                IpNetwork::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 0)), 8).unwrap(),
                // forbid ipv4 link-local 169.254.0.0/16 by default
                IpNetwork::new(IpAddr::V4(Ipv4Addr::new(169, 254, 0, 0)), 16).unwrap(),
                // forbid ipv6 unspecified ::/128 by default
                IpNetwork::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 128).unwrap(),
                // forbid ipv6 loopback ::1/128 by default
                IpNetwork::new(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)), 128).unwrap(),
                // forbid ipv6 link-local fe80::/10 by default
                IpNetwork::new(IpAddr::V6(Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 0)), 10).unwrap(),
                // forbid ipv6 discard-only 100::/64 by default
                IpNetwork::new(IpAddr::V6(Ipv6Addr::new(0x0100, 0, 0, 0, 0, 0, 0, 0)), 64).unwrap(),
            ]
        });
        let v = DEFAULT_EGRESS_RULE.clone();
        let mut inner = HashMap::with_capacity(v.len());
        for ip_network in v {
            inner.insert(ip_network, Action::default_forbid());
        }
        Self {
            inner,
            missed_action,
        }
    }

    pub fn new_ingress(missed_action: Action) -> Self {
        static DEFAULT_INGRESS_RULE: LazyLock<Vec<IpNetwork>> = LazyLock::new(|| {
            vec![
                // permit ipv4 loopback 127.0.0.1/32 by default
                IpNetwork::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)), 32).unwrap(),
                // permit ipv6 loopback ::1/128 by default
                IpNetwork::new(IpAddr::V6(Ipv6Addr::new(0, 0, 0, 0, 0, 0, 0, 1)), 128).unwrap(),
            ]
        });
        let v = DEFAULT_INGRESS_RULE.clone();
        let mut inner = HashMap::with_capacity(v.len());
        for ip_network in v {
            inner.insert(ip_network, Action::default_permit());
        }
        Self {
            inner,
            missed_action,
        }
    }

    #[inline]
    pub fn add_network(&mut self, network: IpNetwork, action: Action) {
        self.inner.insert(network, action);
    }

    #[inline]
    pub fn missed_action(&self) -> Action {
        self.missed_action
    }

    #[inline]
    pub fn set_missed_action(&mut self, action: Action) {
        self.missed_action = action;
    }

    pub fn build(&self) -> AclNetworkRule<Action> {
        let mut inner = IpNetworkTable::new();
        for (net, action) in &self.inner {
            inner.insert(*net, *action);
        }
        AclNetworkRule {
            inner,
            default_action: self.missed_action,
        }
    }
}

pub struct AclNetworkRule<Action = AclAction> {
    inner: IpNetworkTable<Action>,
    default_action: Action,
}

impl<Action: ActionContract> AclNetworkRule<Action> {
    pub fn check(&self, ip: IpAddr) -> (bool, Action) {
        if let Some((_, action)) = self.inner.longest_match(ip) {
            (true, *action)
        } else {
            (false, self.default_action)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    #[test]
    fn check() {
        let mut builder = AclNetworkRuleBuilder::new_egress(AclAction::Permit);
        builder.add_network(
            IpNetwork::from_str("192.168.1.0/24").unwrap(),
            AclAction::Forbid,
        );
        builder.add_network(
            IpNetwork::from_str("2001:1:2:3::/64").unwrap(),
            AclAction::PermitAndLog,
        );
        builder.add_network(
            IpNetwork::from_str("192.168.30.1/32").unwrap(),
            AclAction::PermitAndLog,
        );

        let rule = builder.build();

        assert_eq!(
            rule.check(IpAddr::from_str("192.168.1.1").unwrap(),),
            (true, AclAction::Forbid)
        );
        assert_eq!(
            rule.check(IpAddr::from_str("127.0.0.1").unwrap()),
            (true, AclAction::Forbid)
        );
        assert_eq!(
            rule.check(IpAddr::from_str("2001:1:2:3::100").unwrap()),
            (true, AclAction::PermitAndLog)
        );
        assert_eq!(
            rule.check(IpAddr::from_str("192.168.30.1").unwrap()),
            (true, AclAction::PermitAndLog)
        );
        assert_eq!(
            rule.check(IpAddr::from_str("1.1.1.1").unwrap()),
            (false, AclAction::Permit)
        )
    }
}
