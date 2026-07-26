/*
 * SPDX-License-Identifier: Apache-2.0
 * SPDX-FileCopyrightText: 2023-2025 ByteDance and/or its affiliates.
 * SPDX-FileCopyrightText: 2026 VEY-OSS Developers.
 */

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use c_ares::{AAAAResults, AResults};
use c_ares_resolver::{CAresFuture, FutureResolver};
use tokio::sync::mpsc;

use crate::config::ResolverRuntimeConfig;
use crate::message::ResolveDriverResponse;
use crate::{ResolveDriver, ResolveError, ResolvedRecord};

pub(super) struct CAresResolver {
    pub(super) inner: FutureResolver,
    pub(super) negative_ttl: u32,
    pub(super) positive_min_ttl: u32,
    pub(super) positive_max_ttl: u32,
}

#[derive(Clone, Copy)]
struct JobConfig {
    timeout: Duration,
    negative_ttl: u32,
    positive_min_ttl: u32,
    positive_max_ttl: u32,
}

impl CAresResolver {
    fn build_job_config(&self, rc: &ResolverRuntimeConfig) -> JobConfig {
        JobConfig {
            timeout: rc.protective_query_timeout,
            negative_ttl: self.negative_ttl,
            positive_min_ttl: self.positive_min_ttl,
            positive_max_ttl: self.positive_max_ttl,
        }
    }
}

trait ResultConverter {
    fn finalize(self) -> (u32, Vec<IpAddr>);
}

/// RFC 2181: an RRset with inconsistent TTLs should use the minimum.
/// Negative TTLs (possible from c-ares) are clamped to 0; `ResolvedRecord::resolved`
/// then raises them to `positive_min_ttl`.
fn min_ttl_u32(ttls: impl IntoIterator<Item = i32>) -> u32 {
    let mut min_ttl: Option<i32> = None;
    for ttl in ttls {
        min_ttl = Some(match min_ttl {
            Some(t) => t.min(ttl),
            None => ttl,
        });
    }
    min_ttl
        .map(|t| u32::try_from(t).unwrap_or(0))
        .unwrap_or(0)
}

impl ResultConverter for AResults {
    fn finalize(self) -> (u32, Vec<IpAddr>) {
        let (ttls, addrs): (Vec<_>, Vec<_>) = self
            .iter()
            .map(|r| (r.ttl(), IpAddr::V4(r.ipv4())))
            .unzip();
        (min_ttl_u32(ttls), addrs)
    }
}

impl ResultConverter for AAAAResults {
    fn finalize(self) -> (u32, Vec<IpAddr>) {
        let (ttls, addrs): (Vec<_>, Vec<_>) = self
            .iter()
            .map(|r| (r.ttl(), IpAddr::V6(r.ipv6())))
            .unzip();
        (min_ttl_u32(ttls), addrs)
    }
}

async fn resolve<T>(
    query_future: CAresFuture<T>,
    domain: Arc<str>,
    config: JobConfig,
) -> ResolvedRecord
where
    T: ResultConverter,
{
    match query_future.await {
        Ok(results) => {
            let (ttl, addrs) = results.finalize();
            ResolvedRecord::resolved(
                domain,
                ttl,
                config.positive_min_ttl,
                config.positive_max_ttl,
                addrs,
            )
        }
        Err(e) => {
            if let Some(e) = ResolveError::from_cares_error(e) {
                ResolvedRecord::failed(domain, config.negative_ttl, e)
            } else {
                ResolvedRecord::empty(domain, config.negative_ttl)
            }
        }
    }
}

async fn resolve_protective<T>(
    query_future: CAresFuture<T>,
    domain: Arc<str>,
    config: JobConfig,
) -> ResolvedRecord
where
    T: ResultConverter,
{
    tokio::time::timeout(
        config.timeout,
        resolve(query_future, domain.clone(), config),
    )
    .await
    .unwrap_or_else(|_| ResolvedRecord::timed_out(domain, config.negative_ttl))
}

impl ResolveDriver for CAresResolver {
    fn query_v4(
        &self,
        domain: Arc<str>,
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
        domain: Arc<str>,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn min_ttl_u32_takes_minimum() {
        assert_eq!(min_ttl_u32([3600, 60, 300]), 60);
        assert_eq!(min_ttl_u32([60]), 60);
        assert_eq!(min_ttl_u32([]), 0);
    }

    #[test]
    fn min_ttl_u32_clamps_negative_to_zero() {
        assert_eq!(min_ttl_u32([-1]), 0);
        assert_eq!(min_ttl_u32([300, -5, 60]), 0);
    }
}
