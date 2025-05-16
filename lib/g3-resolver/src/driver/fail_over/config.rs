/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::cmp::PartialEq;
use std::time::Duration;

use super::FailOverResolver;
use crate::{BoxResolverDriver, ResolverHandle};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FailOverDriverStaticConfig {
    pub(crate) fallback_delay: Duration,
    pub(crate) negative_ttl: u32,
    pub(crate) retry_empty_record: bool,
}

impl Default for FailOverDriverStaticConfig {
    fn default() -> Self {
        FailOverDriverStaticConfig {
            fallback_delay: Duration::from_millis(100),
            negative_ttl: crate::config::RESOLVER_MINIMUM_CACHE_TTL,
            retry_empty_record: false,
        }
    }
}

impl FailOverDriverStaticConfig {
    pub fn fallback_delay(&mut self, timeout: Duration) {
        self.fallback_delay = timeout;
    }

    pub fn set_negative_ttl(&mut self, ttl: u32) {
        self.negative_ttl = ttl;
    }

    pub fn set_retry_empty_record(&mut self, retry: bool) {
        self.retry_empty_record = retry;
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct FailOverDriverConfig {
    primary_handle: Option<ResolverHandle>,
    standby_handle: Option<ResolverHandle>,
    static_config: FailOverDriverStaticConfig,
}

impl FailOverDriverConfig {
    pub fn set_primary_handle(&mut self, handle: Option<ResolverHandle>) {
        self.primary_handle = handle;
    }

    pub fn set_standby_handle(&mut self, handle: Option<ResolverHandle>) {
        self.standby_handle = handle;
    }

    pub fn set_static_config(&mut self, conf: FailOverDriverStaticConfig) {
        self.static_config = conf;
    }

    pub(crate) fn spawn_resolver_driver(&self) -> BoxResolverDriver {
        Box::new(FailOverResolver {
            primary: self.primary_handle.clone(),
            standby: self.standby_handle.clone(),
            conf: self.static_config,
        })
    }
}
