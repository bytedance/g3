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

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use anyhow::Context;

use g3_types::collection::{SelectivePickPolicy, SelectiveVec, SelectiveVecBuilder, WeightedValue};

use crate::config::server::openssl_proxy::OpensslServiceConfig;

pub(crate) struct OpensslService {
    addrs: SelectiveVec<WeightedValue<SocketAddr>>,
    pick_policy: SelectivePickPolicy,
}

impl TryFrom<&Arc<OpensslServiceConfig>> for OpensslService {
    type Error = anyhow::Error;

    fn try_from(value: &Arc<OpensslServiceConfig>) -> Result<Self, Self::Error> {
        OpensslService::build(value)
    }
}

impl OpensslService {
    fn build(config: &Arc<OpensslServiceConfig>) -> anyhow::Result<Self> {
        let mut builder = SelectiveVecBuilder::new();
        for v in &config.addrs {
            builder.insert(*v);
        }
        let addrs = builder.build().context("failed to build selective vec")?;
        Ok(OpensslService {
            addrs,
            pick_policy: config.pick_policy,
        })
    }

    pub(super) fn select_addr(&self, peer_ip: IpAddr) -> SocketAddr {
        let addr = match self.pick_policy {
            SelectivePickPolicy::Random => self.addrs.pick_random(),
            SelectivePickPolicy::Serial => self.addrs.pick_serial(),
            SelectivePickPolicy::RoundRobin => self.addrs.pick_round_robin(),
            SelectivePickPolicy::JumpHash => self.addrs.pick_jump(&peer_ip),
            SelectivePickPolicy::Rendezvous => self.addrs.pick_rendezvous(&peer_ip),
        };
        *addr.inner()
    }
}
