/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::collections::hash_map;
use std::io;
use std::net::IpAddr;
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
    request_batch_handle_count: usize,
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

            if let Some(ip) = ip
                && let Some(vec) = self.doing.remove(&ip)
            {
                for req in vec.into_iter() {
                    let _ = req.notifier.send(location.clone());
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
            // if no new value found, just use the old expired value
            if let Some((_net, v)) = self.cache.longest_match(ip)
                && let Some(vec) = self.doing.remove(&ip)
            {
                for req in vec.into_iter() {
                    let _ = req.notifier.send(v.location.clone());
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
        if let Some((_net, v)) = self.cache.longest_match(req.ip)
            && v.valid_before >= Instant::now()
        {
            let _ = req.notifier.send(v.location.clone());
            return;
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
            for _ in 1..self.request_batch_handle_count {
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
