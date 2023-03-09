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

use std::convert::TryFrom;
use std::net::IpAddr;
use std::time::Duration;

use c_ares::{AAAAResults, AResults};
use c_ares_resolver::{CAresFuture, FutureResolver};
use tokio::sync::mpsc;
use tokio::time::Instant;

use crate::config::ResolverRuntimeConfig;
use crate::message::ResolveDriverResponse;
use crate::{ResolveDriver, ResolveError, ResolvedRecord};

pub(super) struct CAresResolver {
    pub(super) inner: FutureResolver,
    pub(super) negative_ttl: u32,
    pub(super) positive_ttl: u32,
}

#[derive(Clone, Copy)]
struct JobConfig {
    timeout: Duration,
    negative_ttl: u32,
    positive_ttl: u32,
}

impl CAresResolver {
    fn build_job_config(&self, rc: &ResolverRuntimeConfig) -> JobConfig {
        JobConfig {
            timeout: rc.protective_query_timeout,
            negative_ttl: self.negative_ttl,
            positive_ttl: self.positive_ttl,
        }
    }
}

trait ResultConverter {
    fn finalize(self) -> (u32, Vec<IpAddr>);
}

impl ResultConverter for AResults {
    fn finalize(self) -> (u32, Vec<IpAddr>) {
        let mut ttl: i32 = 0; // see rfc2181
        let mut addrs = Vec::<IpAddr>::new();
        for result in self.iter() {
            ttl = result.ttl();
            addrs.push(IpAddr::V4(result.ipv4()));
        }
        let ttl = u32::try_from(ttl).unwrap_or_default();

        (ttl, addrs)
    }
}

impl ResultConverter for AAAAResults {
    fn finalize(self) -> (u32, Vec<IpAddr>) {
        let mut ttl: i32 = 0; // see rfc2181
        let mut addrs = Vec::<IpAddr>::new();
        for result in self.iter() {
            ttl = result.ttl();
            addrs.push(IpAddr::V6(result.ipv6()));
        }
        let ttl = u32::try_from(ttl).unwrap_or_default();

        (ttl, addrs)
    }
}

async fn resolve<T>(query_future: CAresFuture<T>, domain: &str, config: JobConfig) -> ResolvedRecord
where
    T: ResultConverter,
{
    let created = Instant::now();
    match query_future.await {
        Ok(results) => {
            let (ttl, addrs) = results.finalize();

            let ttl = if config.negative_ttl < config.positive_ttl {
                ttl.clamp(config.negative_ttl, config.positive_ttl)
            } else {
                ttl.min(config.positive_ttl)
            };

            let expire = created.checked_add(Duration::from_secs(ttl as u64));
            ResolvedRecord {
                domain: domain.to_string(),
                created,
                expire,
                result: Ok(addrs),
            }
        }
        Err(e) => {
            let expire = created.checked_add(Duration::from_secs(config.negative_ttl as u64));
            if let Some(e) = ResolveError::from_cares_error(e) {
                ResolvedRecord {
                    domain: domain.to_string(),
                    created,
                    expire,
                    result: Err(e),
                }
            } else {
                ResolvedRecord {
                    domain: domain.to_string(),
                    created,
                    expire,
                    result: Ok(Vec::new()),
                }
            }
        }
    }
}

async fn resolve_protective<T>(
    query_future: CAresFuture<T>,
    domain: String,
    config: JobConfig,
) -> ResolvedRecord
where
    T: ResultConverter,
{
    tokio::time::timeout(config.timeout, resolve(query_future, &domain, config))
        .await
        .unwrap_or_else(|_| ResolvedRecord::timed_out(domain, config.negative_ttl))
}

impl ResolveDriver for CAresResolver {
    fn query_v4(
        &self,
        domain: String,
        config: &ResolverRuntimeConfig,
        sender: mpsc::UnboundedSender<ResolveDriverResponse>,
    ) {
        let job_config = self.build_job_config(config);
        let query = self.inner.query_a(&domain);
        tokio::spawn(async move {
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
        let job_config = self.build_job_config(config);
        let query = self.inner.query_aaaa(&domain);
        tokio::spawn(async move {
            let record = resolve_protective(query, domain, job_config).await;

            let _ = sender.send(ResolveDriverResponse::V6(record)); // TODO log error
        });
    }
}
