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

use std::sync::Arc;
use std::time::Duration;

use capnp::capability::Promise;
use capnp_rpc::pry;

use g3_types::resolve::{QueryStrategy as ResolveQueryStrategy, ResolveStrategy};

use g3proxy_proto::resolver_capnp::{resolver_control, QueryStrategy};

use crate::resolve::{ArcIntegratedResolverHandle, HappyEyeballsResolveJob};

pub(super) struct ResolverControlImpl {
    resolver_handler: ArcIntegratedResolverHandle,
}

impl ResolverControlImpl {
    pub(super) fn new_client(name: &str) -> anyhow::Result<resolver_control::Client> {
        let handler = crate::resolve::get_handle(name)?;
        Ok(capnp_rpc::new_client(ResolverControlImpl {
            resolver_handler: handler,
        }))
    }
}

impl resolver_control::Server for ResolverControlImpl {
    fn query(
        &mut self,
        params: resolver_control::QueryParams,
        mut results: resolver_control::QueryResults,
    ) -> Promise<(), capnp::Error> {
        let params = pry!(params.get());
        let domain = pry!(params.get_domain()).to_string();
        let resolution_delay = params.get_resolution_delay() as u64;
        let query_strategy = pry!(params.get_strategy());
        let resolver_strategy = get_resolver_strategy(query_strategy);
        let resolver_handler = Arc::clone(&self.resolver_handler);

        Promise::from_future(async move {
            let mut job =
                HappyEyeballsResolveJob::new_dyn(resolver_strategy, &resolver_handler, &domain)
                    .map_err(|e| capnp::Error {
                        kind: capnp::ErrorKind::Failed,
                        description: format!("{e:?}"),
                    })?; // FIXME use api error (via results) instead of rpc error ?
            match job
                .get_r1_or_first(Duration::from_millis(resolution_delay), usize::MAX)
                .await
            {
                Ok(ips) => {
                    let mut ips_builder = results.get().init_result().init_ip(ips.len() as u32);
                    for (i, ip) in ips.iter().enumerate() {
                        ips_builder.set(i as u32, &ip.to_string());
                    }
                }
                Err(e) => results.get().init_result().set_err(&format!("{e:?}")),
            }
            Ok(())
        })
    }
}

fn get_resolver_strategy(q: QueryStrategy) -> ResolveStrategy {
    let qs = match q {
        QueryStrategy::Ipv4First => ResolveQueryStrategy::Ipv4First,
        QueryStrategy::Ipv6First => ResolveQueryStrategy::Ipv6First,
        QueryStrategy::Ipv4Only => ResolveQueryStrategy::Ipv4Only,
        QueryStrategy::Ipv6Only => ResolveQueryStrategy::Ipv6Only,
    };
    ResolveStrategy {
        query: qs,
        pick: Default::default(),
    }
}
