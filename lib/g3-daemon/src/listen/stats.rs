/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::sync::atomic::{AtomicIsize, AtomicU64, Ordering};

use g3_io_ext::haproxy::ProxyProtocolReadError;
use g3_types::metrics::NodeName;
use g3_types::stats::StatId;

#[derive(Default)]
pub struct ListenSnapshot {
    pub accepted: u64,
    pub dropped: u64,
    pub timeout: u64,
    pub failed: u64,
}

#[derive(Debug)]
pub struct ListenStats {
    name: NodeName,
    id: StatId,

    runtime_count: AtomicIsize,
    accepted: AtomicU64,
    dropped: AtomicU64,
    timeout: AtomicU64,
    failed: AtomicU64,
}

impl ListenStats {
    pub fn new(name: &NodeName) -> Self {
        ListenStats {
            name: name.clone(),
            id: StatId::new_unique(),
            runtime_count: AtomicIsize::new(0),
            accepted: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
            timeout: AtomicU64::new(0),
            failed: AtomicU64::new(0),
        }
    }

    #[inline]
    pub fn name(&self) -> &NodeName {
        &self.name
    }

    #[inline]
    pub fn stat_id(&self) -> StatId {
        self.id
    }

    #[must_use]
    pub fn add_running_runtime(self: &Arc<Self>) -> ListenAliveGuard {
        self.runtime_count.fetch_add(1, Ordering::Relaxed);
        ListenAliveGuard(self.clone())
    }

    pub fn running_runtime_count(&self) -> isize {
        self.runtime_count.load(Ordering::Relaxed)
    }
    #[inline]
    pub fn is_running(&self) -> bool {
        self.running_runtime_count() > 0
    }

    pub fn add_accepted(&self) {
        self.accepted.fetch_add(1, Ordering::Relaxed);
    }
    pub fn accepted(&self) -> u64 {
        self.accepted.load(Ordering::Relaxed)
    }

    pub fn add_dropped(&self) {
        self.dropped.fetch_add(1, Ordering::Relaxed);
    }
    pub fn dropped(&self) -> u64 {
        self.dropped.load(Ordering::Relaxed)
    }

    pub fn add_timeout(&self) {
        self.timeout.fetch_add(1, Ordering::Relaxed);
    }
    pub fn timeout(&self) -> u64 {
        self.timeout.load(Ordering::Relaxed)
    }

    pub fn add_failed(&self) {
        self.failed.fetch_add(1, Ordering::Relaxed);
    }
    pub fn failed(&self) -> u64 {
        self.failed.load(Ordering::Relaxed)
    }

    pub fn add_by_proxy_protocol_error(&self, e: ProxyProtocolReadError) {
        match e {
            ProxyProtocolReadError::ReadTimeout => self.add_timeout(),
            ProxyProtocolReadError::ReadFailed(_) | ProxyProtocolReadError::ClosedUnexpected => {
                self.add_failed()
            }
            ProxyProtocolReadError::InvalidMagicHeader
            | ProxyProtocolReadError::InvalidDataLength(_)
            | ProxyProtocolReadError::InvalidVersion(_)
            | ProxyProtocolReadError::InvalidCommand(_)
            | ProxyProtocolReadError::InvalidFamily(_)
            | ProxyProtocolReadError::InvalidProtocol(_)
            | ProxyProtocolReadError::InvalidSrcAddr
            | ProxyProtocolReadError::InvalidDstAddr => self.add_dropped(),
        }
    }
}

pub struct ListenAliveGuard(Arc<ListenStats>);

impl Drop for ListenAliveGuard {
    fn drop(&mut self) {
        self.0.runtime_count.fetch_sub(1, Ordering::Relaxed);
    }
}
