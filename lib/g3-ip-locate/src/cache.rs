/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::collections::hash_map;
use std::io;
use std::net::IpAddr;
use std::num::NonZeroUsize;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use ip_network_table::IpNetworkTable;
use rustc_hash::FxHashMap;
use tokio::sync::mpsc;
use tokio::time::Instant;

use g3_geoip_types::IpLocation;

use super::{CacheQueryRequest, IpLocateServiceConfig, IpLocationCacheResponse};

struct CacheValue {
    valid_before: Instant,
    location: Arc<IpLocation>,
}

pub(crate) struct IpLocationCacheRuntime {
    request_batch_handle_count: NonZeroUsize,
    cache: IpNetworkTable<CacheValue>,
    doing: FxHashMap<IpAddr, Vec<CacheQueryRequest>>,
    req_receiver: mpsc::UnboundedReceiver<CacheQueryRequest>,
    rsp_receiver: mpsc::UnboundedReceiver<(Option<IpAddr>, IpLocationCacheResponse)>,
    query_sender: mpsc::UnboundedSender<IpAddr>,
}

impl IpLocationCacheRuntime {
    pub(crate) fn new(
        config: &IpLocateServiceConfig,
        req_receiver: mpsc::UnboundedReceiver<CacheQueryRequest>,
        rsp_receiver: mpsc::UnboundedReceiver<(Option<IpAddr>, IpLocationCacheResponse)>,
        query_sender: mpsc::UnboundedSender<IpAddr>,
    ) -> Self {
        IpLocationCacheRuntime {
            request_batch_handle_count: config.cache_request_batch_count,
            cache: IpNetworkTable::new(),
            doing: FxHashMap::default(),
            req_receiver,
            rsp_receiver,
            query_sender,
        }
    }

    fn handle_rsp(&mut self, ip: Option<IpAddr>, mut rsp: IpLocationCacheResponse) {
        if let Some(location) = rsp.value.take() {
            let net = location.network_addr();
            let location = Arc::new(location);

            if let Some(ip) = ip {
                if let Some(vec) = self.doing.remove(&ip) {
                    for req in vec.into_iter() {
                        let _ = req.notifier.send(location.clone());
                    }
                }
            }

            // also allow push if no doing ip found
            self.cache.insert(
                net,
                CacheValue {
                    valid_before: rsp.expire_at,
                    location,
                },
            );
        } else if let Some(ip) = ip {
            // Always clear doing so a miss (no geo data) cannot pin the IP forever.
            // Prefer a stale cache entry when present; otherwise drop notifiers so
            // waiters observe Err and map to None.
            if let Some(vec) = self.doing.remove(&ip) {
                if let Some((_net, v)) = self.cache.longest_match(ip) {
                    for req in vec.into_iter() {
                        let _ = req.notifier.send(v.location.clone());
                    }
                }
            }
        }
    }

    fn send_req(&mut self, ip: IpAddr) {
        if self.query_sender.send(ip).is_err() {
            // the query runtime should not close before the cache runtime
            unreachable!()
        }
    }

    fn handle_req(&mut self, req: CacheQueryRequest) {
        if let Some((_net, v)) = self.cache.longest_match(req.ip) {
            if v.valid_before >= Instant::now() {
                let _ = req.notifier.send(v.location.clone());
                return;
            }
        }

        match self.doing.entry(req.ip) {
            hash_map::Entry::Occupied(mut o) => {
                o.get_mut().push(req);
            }
            hash_map::Entry::Vacant(v) => {
                let ip = req.ip;
                v.insert(vec![req]);
                self.send_req(ip);
            }
        }
    }

    fn poll_loop(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        loop {
            // handle rsp
            loop {
                match self.rsp_receiver.poll_recv(cx) {
                    Poll::Pending => break,
                    Poll::Ready(None) => unreachable!(), // unreachable as we have kept a sender
                    Poll::Ready(Some((ip, rsp))) => self.handle_rsp(ip, rsp),
                }
            }

            // handle req
            for _ in 0..self.request_batch_handle_count.get() {
                match self.req_receiver.poll_recv(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(None) => return Poll::Ready(Ok(())),
                    Poll::Ready(Some(req)) => self.handle_req(req),
                }
            }
        }
    }
}

impl Future for IpLocationCacheRuntime {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        (*self).poll_loop(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;
    use std::time::Duration;

    use tokio::sync::oneshot;

    use crate::IpLocationCacheResponse;

    #[tokio::test]
    async fn empty_response_without_cache_clears_doing_and_allows_requery() {
        let config = IpLocateServiceConfig::default();
        let (rsp_sender, rsp_receiver) = mpsc::unbounded_channel();
        let (query_sender, mut query_receiver) = mpsc::unbounded_channel();
        let (req_sender, req_receiver) = mpsc::unbounded_channel();
        let runtime =
            IpLocationCacheRuntime::new(&config, req_receiver, rsp_receiver, query_sender);
        let runtime_task = tokio::spawn(runtime);

        let ip = IpAddr::V4(Ipv4Addr::new(1, 2, 3, 4));
        let (notifier, waiter) = oneshot::channel();
        req_sender
            .send(CacheQueryRequest { ip, notifier })
            .unwrap();

        let queried = tokio::time::timeout(Duration::from_secs(1), query_receiver.recv())
            .await
            .expect("timed out waiting for first query")
            .expect("query channel closed");
        assert_eq!(queried, ip);

        rsp_sender
            .send((Some(ip), IpLocationCacheResponse::empty(10)))
            .unwrap();

        assert!(
            waiter.await.is_err(),
            "empty response with no cache should drop notifiers"
        );

        let (notifier2, _waiter2) = oneshot::channel();
        req_sender
            .send(CacheQueryRequest {
                ip,
                notifier: notifier2,
            })
            .unwrap();

        let queried_again = tokio::time::timeout(Duration::from_secs(1), query_receiver.recv())
            .await
            .expect("timed out waiting for re-query after empty response")
            .expect("query channel closed");
        assert_eq!(queried_again, ip);

        drop(req_sender);
        drop(rsp_sender);
        let _ = runtime_task.await;
    }
}
