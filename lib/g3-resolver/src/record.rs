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

use std::fmt;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::time::Instant;

use super::ResolveError;
use crate::ResolveLocalError;

#[derive(Clone, Copy, Debug)]
pub enum ResolvedRecordSource {
    Cache,
    Query,
}

impl ResolvedRecordSource {
    pub const fn as_str(&self) -> &'static str {
        match self {
            ResolvedRecordSource::Cache => "cache",
            ResolvedRecordSource::Query => "query",
        }
    }
}

impl fmt::Display for ResolvedRecordSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

#[derive(Clone, Debug)]
pub struct ResolvedRecord {
    pub domain: String,
    pub created: Instant,
    pub expire: Option<Instant>,
    pub result: Result<Vec<IpAddr>, ResolveError>,
}

pub type ArcResolvedRecord = Arc<ResolvedRecord>;

impl ResolvedRecord {
    pub fn is_ok(&self) -> bool {
        self.result.is_ok()
    }

    pub fn is_err(&self) -> bool {
        self.result.is_err()
    }

    pub fn timed_out(domain: String, protective_cache_ttl: u32) -> Self {
        ResolvedRecord::failed(
            domain,
            protective_cache_ttl,
            ResolveError::FromLocal(ResolveLocalError::DriverTimedOut),
        )
    }

    pub fn failed(domain: String, protective_cache_ttl: u32, err: ResolveError) -> Self {
        let created = Instant::now();
        let expire = created.checked_add(Duration::from_secs(protective_cache_ttl as u64));
        ResolvedRecord {
            domain,
            created,
            expire,
            result: Err(err),
        }
    }
}
