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

use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use super::{
    ResolveDriverError, ResolveError, ResolveLocalError, ResolveServerError, ResolvedRecord,
};

#[derive(Default)]
pub struct ResolverQueryStats {
    query_total: AtomicU64,
    query_cached: AtomicU64,
    query_driver: AtomicU64,
    driver_timeout: AtomicU64,
    driver_refused: AtomicU64,
    driver_malformed: AtomicU64,
    server_refused: AtomicU64,
    server_malformed: AtomicU64,
    server_not_found: AtomicU64,
    server_serv_fail: AtomicU64,
}

#[derive(Default)]
pub struct ResolverQuerySnapshot {
    pub total: u64,
    pub cached: u64,
    pub driver: u64,
    pub driver_timeout: u64,
    pub driver_refused: u64,
    pub driver_malformed: u64,
    pub server_refused: u64,
    pub server_malformed: u64,
    pub server_not_found: u64,
    pub server_serv_fail: u64,
}

impl ResolverQueryStats {
    fn snapshot(&self) -> ResolverQuerySnapshot {
        ResolverQuerySnapshot {
            total: self.query_total.load(Ordering::Relaxed),
            cached: self.query_cached.load(Ordering::Relaxed),
            driver: self.query_driver.load(Ordering::Relaxed),
            driver_timeout: self.driver_timeout.load(Ordering::Relaxed),
            driver_refused: self.driver_refused.load(Ordering::Relaxed),
            driver_malformed: self.driver_malformed.load(Ordering::Relaxed),
            server_refused: self.server_refused.load(Ordering::Relaxed),
            server_malformed: self.server_malformed.load(Ordering::Relaxed),
            server_not_found: self.server_not_found.load(Ordering::Relaxed),
            server_serv_fail: self.server_serv_fail.load(Ordering::Relaxed),
        }
    }

    pub(crate) fn add_query_total(&self) {
        self.query_total.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_query_cached(&self) {
        self.query_cached.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_query_cached_n(&self, n: usize) {
        if n > 0 {
            self.query_cached.fetch_add(n as u64, Ordering::Relaxed);
        }
    }

    pub(crate) fn add_query_driver(&self) {
        self.query_driver.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    fn add_driver_timeout(&self) {
        self.driver_timeout.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    fn add_driver_refused(&self) {
        self.driver_refused.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    fn add_driver_malformed(&self) {
        self.driver_malformed.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    fn add_server_refused(&self) {
        self.server_refused.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    fn add_server_malformed(&self) {
        self.server_malformed.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    fn add_server_not_found(&self) {
        self.server_not_found.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    fn add_server_serv_fail(&self) {
        self.server_serv_fail.fetch_add(1, Ordering::Relaxed);
    }

    pub(crate) fn add_record(&self, record: &ResolvedRecord) {
        if let Err(e) = &record.result {
            self.add_error(e);
        }
    }

    fn add_server_error(&self, e: &ResolveServerError) {
        match e {
            ResolveServerError::Refused => self.add_server_refused(),
            ResolveServerError::FormErr => self.add_server_malformed(),
            ResolveServerError::NotFound => self.add_server_not_found(),
            ResolveServerError::ServFail => self.add_server_serv_fail(),
            _ => {}
        }
    }

    fn add_driver_error(&self, e: &ResolveDriverError) {
        match e {
            ResolveDriverError::ConnRefused => self.add_driver_refused(),
            ResolveDriverError::Timeout => self.add_driver_timeout(),
            ResolveDriverError::BadName
            | ResolveDriverError::BadQuery
            | ResolveDriverError::BadResp => self.add_driver_malformed(),
            _ => {}
        }
    }

    pub(crate) fn add_error(&self, e: &ResolveError) {
        match e {
            ResolveError::FromServer(e) => self.add_server_error(e),
            ResolveError::FromDriver(e) => self.add_driver_error(e),
            ResolveError::FromLocal(ResolveLocalError::DriverTimedOut) => self.add_driver_timeout(),
            _ => {}
        }
    }
}

#[derive(Default)]
pub(crate) struct ResolverMemoryStats {
    cap_cache: AtomicUsize,
    len_cache: AtomicUsize,
    cap_doing: AtomicUsize,
    len_doing: AtomicUsize,
}

#[derive(Default)]
pub struct ResolverMemorySnapshot {
    pub cap_cache: usize,
    pub len_cache: usize,
    pub cap_doing: usize,
    pub len_doing: usize,
}

impl ResolverMemoryStats {
    fn snapshot(&self) -> ResolverMemorySnapshot {
        ResolverMemorySnapshot {
            cap_cache: self.cap_cache.load(Ordering::Relaxed),
            len_cache: self.len_cache.load(Ordering::Relaxed),
            cap_doing: self.cap_doing.load(Ordering::Relaxed),
            len_doing: self.len_doing.load(Ordering::Relaxed),
        }
    }

    pub(crate) fn set_cache_capacity(&self, value: usize) {
        self.cap_cache.store(value, Ordering::Relaxed);
    }

    pub(crate) fn set_cache_length(&self, value: usize) {
        self.len_cache.store(value, Ordering::Relaxed);
    }

    pub(crate) fn set_doing_capacity(&self, value: usize) {
        self.cap_doing.store(value, Ordering::Relaxed);
    }

    pub(crate) fn set_doing_length(&self, value: usize) {
        self.len_doing.store(value, Ordering::Relaxed);
    }
}

#[derive(Default)]
pub struct ResolverStats {
    pub(crate) query_a: ResolverQueryStats,
    pub(crate) query_aaaa: ResolverQueryStats,
    pub(crate) memory_a: ResolverMemoryStats,
    pub(crate) memory_aaaa: ResolverMemoryStats,
}

impl ResolverStats {
    pub fn snapshot(&self) -> ResolverSnapshot {
        ResolverSnapshot {
            query_a: self.query_a.snapshot(),
            query_aaaa: self.query_aaaa.snapshot(),
            memory_a: self.memory_a.snapshot(),
            memory_aaaa: self.memory_aaaa.snapshot(),
        }
    }
}

#[derive(Default)]
pub struct ResolverSnapshot {
    pub query_a: ResolverQuerySnapshot,
    pub query_aaaa: ResolverQuerySnapshot,
    pub memory_a: ResolverMemorySnapshot,
    pub memory_aaaa: ResolverMemorySnapshot,
}
