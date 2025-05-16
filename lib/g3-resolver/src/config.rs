/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::time::Duration;

use super::AnyResolveDriverConfig;

pub(crate) const RESOLVER_MINIMUM_CACHE_TTL: u32 = 30;
#[cfg(any(feature = "c-ares", feature = "hickory"))]
pub(crate) const RESOLVER_MAXIMUM_CACHE_TTL: u32 = 3600;
#[cfg(any(feature = "c-ares", feature = "hickory"))]
pub(crate) const RESOLVER_DELETE_CACHE_TTL: u32 = 7200;

const RESOLVER_CACHE_INITIAL_CAPACITY: usize = 10;
const RESOLVER_BATCH_REQUEST_COUNT: usize = 10;
const RESOLVER_PROTECTIVE_QUERY_TIMEOUT: Duration = Duration::from_secs(60);
const RESOLVER_GRACEFUL_STOP_WAIT: Duration = Duration::from_secs(30);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolverRuntimeConfig {
    pub initial_cache_capacity: usize,
    pub batch_request_count: usize,
    pub protective_query_timeout: Duration,
    pub graceful_stop_wait: Duration,
}

impl Default for ResolverRuntimeConfig {
    fn default() -> Self {
        ResolverRuntimeConfig {
            initial_cache_capacity: RESOLVER_CACHE_INITIAL_CAPACITY,
            batch_request_count: RESOLVER_BATCH_REQUEST_COUNT,
            protective_query_timeout: RESOLVER_PROTECTIVE_QUERY_TIMEOUT,
            graceful_stop_wait: RESOLVER_GRACEFUL_STOP_WAIT,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolverConfig {
    pub name: String,
    pub driver: AnyResolveDriverConfig,
    pub runtime: ResolverRuntimeConfig,
}
