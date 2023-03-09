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

use std::fmt;
use std::str::FromStr;

mod a_hash;
mod child_domain;
mod exact_host;
mod exact_port;
mod fx_hash;
mod network;
mod proxy_request;
mod radix_trie;
mod regex_set;
mod user_agent;

use self::radix_trie::{AclRadixTrieRule, AclRadixTrieRuleBuilder};
use a_hash::AclAHashRule;
use fx_hash::AclFxHashRule;

pub use child_domain::{AclChildDomainRule, AclChildDomainRuleBuilder};
pub use exact_host::AclExactHostRule;
pub use exact_port::AclExactPortRule;
pub use network::{AclNetworkRule, AclNetworkRuleBuilder};
pub use proxy_request::AclProxyRequestRule;
pub use regex_set::{AclRegexSetRule, AclRegexSetRuleBuilder};
pub use user_agent::AclUserAgentRule;

#[derive(Clone, Copy, Debug, Eq, PartialEq, PartialOrd)]
pub enum AclAction {
    Permit,
    PermitAndLog,
    Forbid,
    ForbidAndLog,
}

impl AclAction {
    #[must_use]
    pub fn restrict(self, other: AclAction) -> AclAction {
        if other > self {
            other
        } else {
            self
        }
    }

    pub fn strict_than(&self, other: AclAction) -> bool {
        self.gt(&other)
    }

    pub const fn forbid_early(&self) -> bool {
        match self {
            AclAction::ForbidAndLog | AclAction::Forbid => true,
            AclAction::PermitAndLog | AclAction::Permit => false,
        }
    }

    pub const fn as_str(&self) -> &'static str {
        match self {
            AclAction::Permit => "Permit",
            AclAction::PermitAndLog => "PermitAndLog",
            AclAction::Forbid => "Forbid",
            AclAction::ForbidAndLog => "ForbidAndLog",
        }
    }
}

impl fmt::Display for AclAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl FromStr for AclAction {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "permit" | "allow" | "accept" => Ok(AclAction::Permit),
            "permit_log" | "allow_log" | "accept_log" => Ok(AclAction::PermitAndLog),
            "forbid" | "deny" | "reject" => Ok(AclAction::Forbid),
            "forbid_log" | "deny_log" | "reject_log" => Ok(AclAction::ForbidAndLog),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acl_action_order() {
        assert_eq!(
            AclAction::Permit.restrict(AclAction::PermitAndLog),
            AclAction::PermitAndLog
        );

        assert_eq!(
            AclAction::Forbid.restrict(AclAction::ForbidAndLog),
            AclAction::ForbidAndLog
        );

        assert_eq!(
            AclAction::Permit.restrict(AclAction::Forbid),
            AclAction::Forbid
        );
    }
}
