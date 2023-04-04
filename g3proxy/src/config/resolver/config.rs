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

use g3_types::metrics::MetricsName;
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
    fn name(&self) -> &MetricsName;
    fn position(&self) -> Option<YamlDocPosition>;
    fn resolver_type(&self) -> &'static str;

    fn diff_action(&self, new: &AnyResolverConfig) -> ResolverConfigDiffAction;
    fn dependent_resolver(&self) -> Option<BTreeSet<MetricsName>>;
}

#[derive(Clone)]
pub(crate) enum AnyResolverConfig {
    #[cfg(feature = "c-ares")]
    CAres(c_ares::CAresResolverConfig),
    TrustDns(trust_dns::TrustDnsResolverConfig),
    DenyAll(deny_all::DenyAllResolverConfig),
    FailOver(fail_over::FailOverResolverConfig),
}

macro_rules! impl_transparent0 {
    ($f:tt, $v:ty) => {
        pub(crate) fn $f(&self) -> $v {
            match self {
                #[cfg(feature = "c-ares")]
                AnyResolverConfig::CAres(r) => r.$f(),
                AnyResolverConfig::TrustDns(r) => r.$f(),
                AnyResolverConfig::DenyAll(r) => r.$f(),
                AnyResolverConfig::FailOver(r) => r.$f(),
            }
        }
    };
}

macro_rules! impl_transparent1 {
    ($f:tt, $v:ty, $p:ty) => {
        pub(crate) fn $f(&self, p: $p) -> $v {
            match self {
                #[cfg(feature = "c-ares")]
                AnyResolverConfig::CAres(r) => r.$f(p),
                AnyResolverConfig::TrustDns(r) => r.$f(p),
                AnyResolverConfig::DenyAll(r) => r.$f(p),
                AnyResolverConfig::FailOver(r) => r.$f(p),
            }
        }
    };
}

impl AnyResolverConfig {
    impl_transparent0!(name, &MetricsName);
    impl_transparent0!(position, Option<YamlDocPosition>);
    impl_transparent0!(dependent_resolver, Option<BTreeSet<MetricsName>>);

    impl_transparent1!(diff_action, ResolverConfigDiffAction, &Self);
}
