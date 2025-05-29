/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::fmt;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::time::Instant;

use super::{ResolveError, ResolveServerError};
use crate::ResolveLocalError;

#[derive(Clone, Copy, Debug)]
pub enum ResolvedRecordSource {
    Cache,
    Trash,
    Query,
}

impl ResolvedRecordSource {
    pub const fn as_str(&self) -> &'static str {
        match self {
            ResolvedRecordSource::Cache => "cache",
            ResolvedRecordSource::Trash => "trash",
            ResolvedRecordSource::Query => "query",
        }
    }
}

impl fmt::Display for ResolvedRecordSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, Debug)]
pub struct ResolvedRecord {
    pub domain: Arc<str>,
    pub created: Instant,
    pub expire: Option<Instant>,
    pub vanish: Option<Instant>,
    pub result: Result<Vec<IpAddr>, ResolveError>,
}

pub type ArcResolvedRecord = Arc<ResolvedRecord>;

impl ResolvedRecord {
    pub fn is_ok(&self) -> bool {
        self.result.is_ok()
    }

    pub fn is_usable(&self) -> bool {
        self.result.as_ref().map(|v| !v.is_empty()).unwrap_or(false)
    }

    pub fn is_err(&self) -> bool {
        self.result.is_err()
    }

    pub fn is_expired(&self, now: Instant) -> bool {
        self.expire.map(|expire| now >= expire).unwrap_or(true)
    }

    pub fn is_acceptable(&self) -> bool {
        let Err(e) = self.result.as_ref() else {
            return true;
        };

        matches!(e, ResolveError::FromServer(ResolveServerError::NotFound))
    }

    pub fn timed_out(domain: Arc<str>, protective_cache_ttl: u32) -> Self {
        ResolvedRecord::failed(
            domain,
            protective_cache_ttl,
            ResolveError::FromLocal(ResolveLocalError::DriverTimedOut),
        )
    }

    pub fn resolved(
        domain: Arc<str>,
        ttl: u32,
        min_ttl: u32,
        max_ttl: u32,
        ips: Vec<IpAddr>,
    ) -> Self {
        let created = Instant::now();
        let (expire_ttl, vanish_ttl) = if ttl > max_ttl + min_ttl {
            (max_ttl, ttl)
        } else if ttl > min_ttl + min_ttl {
            (ttl - min_ttl, ttl)
        } else if ttl > min_ttl {
            (min_ttl, ttl)
        } else {
            (min_ttl, min_ttl + 1)
        };
        let expire = created.checked_add(Duration::from_secs(expire_ttl as u64));
        let vanish = created.checked_add(Duration::from_secs(vanish_ttl as u64));
        ResolvedRecord {
            domain,
            created,
            expire,
            vanish,
            result: Ok(ips),
        }
    }

    pub fn empty(domain: Arc<str>, expire_ttl: u32) -> Self {
        let created = Instant::now();
        let expire = created.checked_add(Duration::from_secs(expire_ttl as u64));
        ResolvedRecord {
            domain,
            created,
            expire,
            vanish: None,
            result: Ok(Vec::new()),
        }
    }

    pub fn failed(domain: Arc<str>, protective_cache_ttl: u32, err: ResolveError) -> Self {
        let created = Instant::now();
        let expire = created.checked_add(Duration::from_secs(protective_cache_ttl as u64));
        ResolvedRecord {
            domain,
            created,
            expire,
            vanish: None,
            result: Err(err),
        }
    }
}
