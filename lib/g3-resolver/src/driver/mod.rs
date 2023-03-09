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

use tokio::sync::mpsc;

use crate::config::ResolverRuntimeConfig;
use crate::message::ResolveDriverResponse;

pub mod fail_over;

#[cfg(feature = "c-ares")]
pub mod c_ares;

#[cfg(feature = "trust-dns")]
pub mod trust_dns;

#[derive(Clone, Debug, PartialEq)]
pub enum AnyResolveDriverConfig {
    FailOver(fail_over::FailOverDriverConfig),
    #[cfg(feature = "c-ares")]
    CAres(c_ares::CAresDriverConfig),
    #[cfg(feature = "trust-dns")]
    TrustDns(trust_dns::TrustDnsDriverConfig),
}

impl AnyResolveDriverConfig {
    pub(crate) fn spawn_resolver_driver(&self) -> anyhow::Result<Box<dyn ResolveDriver>> {
        match self {
            AnyResolveDriverConfig::FailOver(c) => Ok(c.spawn_resolver_driver()),
            #[cfg(feature = "c-ares")]
            AnyResolveDriverConfig::CAres(c) => c.spawn_resolver_driver(),
            #[cfg(feature = "trust-dns")]
            AnyResolveDriverConfig::TrustDns(c) => c.spawn_resolver_driver(),
        }
    }
}

pub(crate) trait ResolveDriver {
    fn query_v4(
        &self,
        domain: String,
        config: &ResolverRuntimeConfig,
        sender: mpsc::UnboundedSender<ResolveDriverResponse>,
    );
    fn query_v6(
        &self,
        domain: String,
        config: &ResolverRuntimeConfig,
        sender: mpsc::UnboundedSender<ResolveDriverResponse>,
    );
}

pub(crate) type BoxResolverDriver = Box<dyn ResolveDriver>;
