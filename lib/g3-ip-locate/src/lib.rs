/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::{mpsc, oneshot};
use tokio::time::Instant;

use g3_geoip_types::IpLocation;

mod config;
pub use config::IpLocateServiceConfig;

mod handle;
pub use handle::IpLocationServiceHandle;
use handle::{IpLocationCacheHandle, IpLocationQueryHandle};

mod cache;
use cache::IpLocationCacheRuntime;

mod query;
use query::IpLocationQueryRuntime;

mod protocol;
pub use protocol::{request_key, request_key_id, response_key, response_key_id};

mod request;
pub use request::Request;

mod response;
pub use response::Response;

mod runtime;
pub use runtime::*;

struct CacheQueryRequest {
    ip: IpAddr,
    notifier: oneshot::Sender<Arc<IpLocation>>,
}

struct IpLocationCacheResponse {
    value: Option<IpLocation>,
    expire_at: Instant,
}

impl IpLocationCacheResponse {
    fn new(location: IpLocation, ttl: u32) -> Self {
        let now = Instant::now();
        let expire_at = now
            .checked_add(Duration::from_secs(ttl as u64))
            .unwrap_or(now);
        IpLocationCacheResponse {
            value: Some(location),
            expire_at,
        }
    }

    fn empty(protective_ttl: u32) -> Self {
        let now = Instant::now();
        let expire_at = now
            .checked_add(Duration::from_secs(protective_ttl as u64))
            .unwrap_or(now);
        IpLocationCacheResponse {
            value: None,
            expire_at,
        }
    }
}

fn crate_ip_location_cache(
    config: &IpLocateServiceConfig,
) -> (
    IpLocationCacheRuntime,
    IpLocationCacheHandle,
    IpLocationQueryHandle,
) {
    let (rsp_sender, rsp_receiver) = mpsc::unbounded_channel();
    let (query_sender, query_receiver) = mpsc::unbounded_channel();
    let (req_sender, req_receiver) = mpsc::unbounded_channel();

    let cache_runtime =
        IpLocationCacheRuntime::new(config, req_receiver, rsp_receiver, query_sender);
    let cache_handle = IpLocationCacheHandle::new(req_sender);
    let query_handle = IpLocationQueryHandle::new(query_receiver, rsp_sender);
    (cache_runtime, cache_handle, query_handle)
}
