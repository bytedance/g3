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

use std::future::Future;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::Instant;
use trust_dns_resolver::lookup::{Ipv4Lookup, Ipv6Lookup};
use trust_dns_resolver::TokioAsyncResolver;

use crate::config::ResolverRuntimeConfig;
use crate::message::ResolveDriverResponse;
use crate::{ResolveDriver, ResolvedRecord};

pub(super) struct TrustDnsResolver {
    pub(super) inner: Arc<TokioAsyncResolver>,
    pub(super) protective_cache_ttl: u32,
}

struct JobConfig {
    timeout: Duration,
    protective_cache_ttl: u32,
}

impl TrustDnsResolver {
    fn build_job_config(&self, rc: &ResolverRuntimeConfig) -> JobConfig {
        JobConfig {
            timeout: rc.protective_query_timeout,
            protective_cache_ttl: self.protective_cache_ttl,
        }
    }
}

trait ResultConverter {
    fn finalize(self) -> (Instant, Vec<IpAddr>);
}

impl ResultConverter for Ipv4Lookup {
    fn finalize(self) -> (Instant, Vec<IpAddr>) {
        let mut addrs = Vec::<IpAddr>::new();
        for ip4 in self.iter() {
            addrs.push(IpAddr::V4(*ip4));
        }

        (Instant::from_std(self.valid_until()), addrs)
    }
}

impl ResultConverter for Ipv6Lookup {
    fn finalize(self) -> (Instant, Vec<IpAddr>) {
        let mut addrs = Vec::<IpAddr>::new();
        for ip6 in self.iter() {
            addrs.push(IpAddr::V6(*ip6));
        }

        (Instant::from_std(self.valid_until()), addrs)
    }
}

async fn resolve_protective<F, T>(
    query_future: F,
    domain: String,
    config: JobConfig,
) -> ResolvedRecord
where
    F: Future<Output = Result<T, trust_dns_resolver::error::ResolveError>>,
    T: ResultConverter,
{
    match tokio::time::timeout(config.timeout, query_future).await {
        Ok(Ok(r)) => {
            let (expire, addrs) = r.finalize();
            ResolvedRecord {
                domain,
                created: Instant::now(),
                expire: Some(expire),
                result: Ok(addrs),
            }
        }
        Ok(Err(e)) => ResolvedRecord::failed(domain, config.protective_cache_ttl, e.into()),
        Err(_) => ResolvedRecord::timed_out(domain, config.protective_cache_ttl),
    }
}

impl ResolveDriver for TrustDnsResolver {
    fn query_v4(
        &self,
        domain: String,
        config: &ResolverRuntimeConfig,
        sender: mpsc::UnboundedSender<ResolveDriverResponse>,
    ) {
        let resolver = Arc::clone(&self.inner);
        let job_config = self.build_job_config(config);
        tokio::spawn(async move {
            let query = resolver.ipv4_lookup(format!("{domain}.")); // add trailing '.' to avoid search
            let record = resolve_protective(query, domain, job_config).await;

            let _ = sender.send(ResolveDriverResponse::V4(record)); // TODO log error
        });
    }

    fn query_v6(
        &self,
        domain: String,
        config: &ResolverRuntimeConfig,
        sender: mpsc::UnboundedSender<ResolveDriverResponse>,
    ) {
        let resolver = Arc::clone(&self.inner);
        let job_config = self.build_job_config(config);
        tokio::spawn(async move {
            let query = resolver.ipv6_lookup(format!("{domain}.")); // add trailing '.' to avoid search
            let record = resolve_protective(query, domain, job_config).await;

            let _ = sender.send(ResolveDriverResponse::V4(record)); // TODO log error
        });
    }
}
