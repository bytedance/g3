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
use std::net::IpAddr;

use anyhow::anyhow;
use ip_network::IpNetwork;

use g3_types::metrics::MetricsName;
use g3_types::net::Host;
use g3_types::resolve::ResolveStrategy;

mod json;
mod yaml;

#[derive(Clone, Default, Debug, PartialEq, Eq)]
pub(crate) struct UserSiteConfig {
    pub(crate) id: MetricsName,
    pub(crate) exact_match_domain: BTreeSet<String>,
    pub(crate) exact_match_ipaddr: BTreeSet<IpAddr>,
    pub(crate) subnet_match_ipaddr: BTreeSet<IpNetwork>,
    pub(crate) child_match_domain: BTreeSet<String>,
    pub(crate) emit_stats: bool,
    pub(crate) resolve_strategy: Option<ResolveStrategy>,
}

impl UserSiteConfig {
    fn check(&self) -> anyhow::Result<()> {
        if self.id.is_empty() {
            return Err(anyhow!("site id is not set"));
        }
        Ok(())
    }

    fn add_exact_host(&mut self, host: Host) {
        match host {
            Host::Domain(domain) => self.exact_match_domain.insert(domain),
            Host::Ip(ip) => self.exact_match_ipaddr.insert(ip),
        };
    }
}
