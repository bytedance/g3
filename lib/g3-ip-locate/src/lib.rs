/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

fn spawn_ip_location_cache(
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
