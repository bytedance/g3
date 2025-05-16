/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use g3_types::acl::{
    AclChildDomainRule, AclChildDomainRuleBuilder, AclExactHostRule, AclNetworkRule,
    AclNetworkRuleBuilder, ActionContract,
};
use g3_types::net::Host;

mod size_limit;

pub use size_limit::ProtocolInspectionSizeLimit;

mod http;
pub use http::{H1InterceptionConfig, H2InterceptionConfig};

mod smtp;
pub use smtp::SmtpInterceptionConfig;

mod imap;
pub use imap::ImapInterceptionConfig;

#[derive(Clone)]
pub struct ProtocolInspectPolicyBuilder {
    missed_action: ProtocolInspectAction,
    pub exact: Option<AclExactHostRule<ProtocolInspectAction>>,
    pub child: Option<AclChildDomainRuleBuilder<ProtocolInspectAction>>,
    pub subnet: Option<AclNetworkRuleBuilder<ProtocolInspectAction>>,
}

impl Default for ProtocolInspectPolicyBuilder {
    fn default() -> Self {
        Self::new(ProtocolInspectAction::Intercept)
    }
}

impl ProtocolInspectPolicyBuilder {
    pub fn new(missed_action: ProtocolInspectAction) -> Self {
        ProtocolInspectPolicyBuilder {
            missed_action,
            exact: None,
            child: None,
            subnet: None,
        }
    }

    pub fn set_missed_action(&mut self, missed_action: ProtocolInspectAction) {
        self.missed_action = missed_action;
    }

    pub fn build(&self) -> ProtocolInspectPolicy {
        ProtocolInspectPolicy {
            exact: self.exact.clone(),
            child: self.child.as_ref().map(|b| b.build()),
            subnet: self.subnet.as_ref().map(|b| b.build()),
            missed_action: self.missed_action,
        }
    }
}

pub struct ProtocolInspectPolicy {
    exact: Option<AclExactHostRule<ProtocolInspectAction>>,
    child: Option<AclChildDomainRule<ProtocolInspectAction>>,
    subnet: Option<AclNetworkRule<ProtocolInspectAction>>,
    missed_action: ProtocolInspectAction,
}

impl ProtocolInspectPolicy {
    pub fn check(&self, upstream: &Host) -> (bool, ProtocolInspectAction) {
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
            }
        }

        (false, self.missed_action)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub enum ProtocolInspectAction {
    Block,
    #[default]
    Intercept,
    Bypass,
    #[cfg(feature = "quic")]
    Detour,
}

impl ProtocolInspectAction {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Intercept => "intercept",
            Self::Bypass => "bypass",
            #[cfg(feature = "quic")]
            Self::Detour => "detour",
        }
    }

    pub fn is_block(&self) -> bool {
        matches!(self, ProtocolInspectAction::Block)
    }
}

impl fmt::Display for ProtocolInspectAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ProtocolInspectAction {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "block" => Ok(ProtocolInspectAction::Block),
            "intercept" => Ok(ProtocolInspectAction::Intercept),
            "bypass" => Ok(ProtocolInspectAction::Bypass),
            #[cfg(feature = "quic")]
            "detour" => Ok(ProtocolInspectAction::Detour),
            _ => Err(()),
        }
    }
}

impl ActionContract for ProtocolInspectAction {}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProtocolInspectionConfig {
    inspect_max_depth: usize,
    data0_buffer_size: usize,
    data0_wait_timeout: Duration,
    data0_read_timeout: Duration,
    data0_size_limit: ProtocolInspectionSizeLimit,
}

impl Default for ProtocolInspectionConfig {
    fn default() -> Self {
        ProtocolInspectionConfig {
            inspect_max_depth: 4,
            data0_buffer_size: 4096,
            data0_wait_timeout: Duration::from_secs(60),
            data0_read_timeout: Duration::from_secs(4),
            data0_size_limit: Default::default(),
        }
    }
}

impl ProtocolInspectionConfig {
    pub fn set_max_depth(&mut self, depth: usize) {
        self.inspect_max_depth = depth;
    }

    #[inline]
    pub fn max_depth(&self) -> usize {
        self.inspect_max_depth
    }

    pub fn set_data0_buffer_size(&mut self, size: usize) {
        self.data0_buffer_size = size;
    }

    #[inline]
    pub fn data0_buffer_size(&self) -> usize {
        self.data0_buffer_size
    }

    #[inline]
    pub fn set_data0_wait_timeout(&mut self, value: Duration) {
        self.data0_wait_timeout = value;
    }

    #[inline]
    pub fn data0_wait_timeout(&self) -> Duration {
        self.data0_wait_timeout
    }

    #[inline]
    pub fn set_data0_read_timeout(&mut self, value: Duration) {
        self.data0_read_timeout = value;
    }

    #[inline]
    pub fn data0_read_timeout(&self) -> Duration {
        self.data0_read_timeout
    }

    #[inline]
    pub fn size_limit(&self) -> &ProtocolInspectionSizeLimit {
        &self.data0_size_limit
    }

    #[inline]
    pub fn size_limit_mut(&mut self) -> &mut ProtocolInspectionSizeLimit {
        &mut self.data0_size_limit
    }
}
