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

use std::cmp::PartialEq;
use std::time::Duration;

use super::FailOverResolver;
use crate::{BoxResolverDriver, ResolverHandle};

#[derive(Clone, Debug, PartialEq)]
pub struct FailOverDriverConfig {
    primary_handle: Option<ResolverHandle>,
    standby_handle: Option<ResolverHandle>,
    timeout: Duration,
    negative_ttl: u32,
}

impl Default for FailOverDriverConfig {
    fn default() -> Self {
        FailOverDriverConfig {
            primary_handle: None,
            standby_handle: None,
            timeout: Duration::from_millis(100),
            negative_ttl: crate::config::RESOLVER_MINIMUM_CACHE_TTL,
        }
    }
}

impl FailOverDriverConfig {
    pub fn set_primary_handle(&mut self, handle: Option<ResolverHandle>) {
        self.primary_handle = handle;
    }

    pub fn set_standby_handle(&mut self, handle: Option<ResolverHandle>) {
        self.standby_handle = handle;
    }

    pub fn set_timeout(&mut self, timeout: Duration) {
        self.timeout = timeout;
    }

    pub fn set_negative_ttl(&mut self, ttl: u32) {
        self.negative_ttl = ttl;
    }

    pub(crate) fn spawn_resolver_driver(&self) -> BoxResolverDriver {
        Box::new(FailOverResolver {
            primary: self.primary_handle.clone(),
            standby: self.standby_handle.clone(),
            timeout: self.timeout,
            negative_ttl: self.negative_ttl,
        })
    }
}
