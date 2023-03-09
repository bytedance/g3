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

use std::collections::BTreeSet;

use g3_yaml::YamlDocPosition;

#[cfg(feature = "c-ares")]
use super::c_ares;
use super::trust_dns;

use super::deny_all;
use super::fail_over;

pub(super) const CONFIG_KEY_RESOLVER_TYPE: &str = "type";
pub(super) const CONFIG_KEY_RESOLVER_NAME: &str = "name";

pub(crate) enum ResolverConfigDiffAction {
    NoAction,
    SpawnNew,
    Update,
}

pub(crate) trait ResolverConfig {
    fn name(&self) -> &str;
    fn position(&self) -> Option<YamlDocPosition>;
    fn resolver_type(&self) -> &'static str;

    fn diff_action(&self, new: &AnyResolverConfig) -> ResolverConfigDiffAction;
    fn dependent_resolver(&self) -> Option<BTreeSet<String>>;
}

#[derive(Clone)]
pub(crate) enum AnyResolverConfig {
    #[cfg(feature = "c-ares")]
    CAres(c_ares::CAresResolverConfig),
    TrustDns(trust_dns::TrustDnsResolverConfig),
    DenyAll(deny_all::DenyAllResolverConfig),
    FailOver(fail_over::FailOverResolverConfig),
}

impl AnyResolverConfig {
    pub(crate) fn name(&self) -> &str {
        match self {
            #[cfg(feature = "c-ares")]
            AnyResolverConfig::CAres(r) => r.name(),
            AnyResolverConfig::TrustDns(r) => r.name(),
            AnyResolverConfig::DenyAll(r) => r.name(),
            AnyResolverConfig::FailOver(r) => r.name(),
        }
    }

    pub(crate) fn position(&self) -> Option<YamlDocPosition> {
        match self {
            #[cfg(feature = "c-ares")]
            AnyResolverConfig::CAres(r) => r.position(),
            AnyResolverConfig::TrustDns(r) => r.position(),
            AnyResolverConfig::DenyAll(r) => r.position(),
            AnyResolverConfig::FailOver(r) => r.position(),
        }
    }

    pub(crate) fn diff_action(&self, new: &Self) -> ResolverConfigDiffAction {
        match self {
            #[cfg(feature = "c-ares")]
            AnyResolverConfig::CAres(r) => r.diff_action(new),
            AnyResolverConfig::TrustDns(r) => r.diff_action(new),
            AnyResolverConfig::DenyAll(r) => r.diff_action(new),
            AnyResolverConfig::FailOver(r) => r.diff_action(new),
        }
    }

    pub(crate) fn dependent_resolver(&self) -> Option<BTreeSet<String>> {
        match self {
            #[cfg(feature = "c-ares")]
            AnyResolverConfig::CAres(r) => r.dependent_resolver(),
            AnyResolverConfig::TrustDns(r) => r.dependent_resolver(),
            AnyResolverConfig::DenyAll(r) => r.dependent_resolver(),
            AnyResolverConfig::FailOver(r) => r.dependent_resolver(),
        }
    }
}
