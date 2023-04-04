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

use std::collections::VecDeque;
use std::future::Future;
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use ahash::AHashMap;
use anyhow::anyhow;
use tokio::io::ReadBuf;
use tokio::net::UdpSocket;
use uuid::Uuid;

use g3_io_ext::{EffectiveCacheData, EffectiveQueryHandle};
use g3_types::collection::{SelectiveVec, SelectiveVecBuilder, WeightedValue};
use g3_types::metrics::MetricsName;

use super::cache::CacheQueryKey;
use super::RouteQueryEscaperConfig;

pub(super) struct QueryRuntime {
    config: Arc<RouteQueryEscaperConfig>,
    socket: UdpSocket,
    query_handle: EffectiveQueryHandle<CacheQueryKey, SelectiveVec<WeightedValue<MetricsName>>>,
    id_key_map: AHashMap<Uuid, Arc<CacheQueryKey>>,
    key_id_map: AHashMap<Arc<CacheQueryKey>, Uuid>,
    read_buffer: Box<[u8]>,
    write_queue: VecDeque<(Arc<CacheQueryKey>, Vec<u8>)>,
}

impl QueryRuntime {
    pub(super) fn new(
        config: &Arc<RouteQueryEscaperConfig>,
        socket: UdpSocket,
        query_handle: EffectiveQueryHandle<CacheQueryKey, SelectiveVec<WeightedValue<MetricsName>>>,
    ) -> Self {
        QueryRuntime {
            config: Arc::clone(config),
            socket,
            query_handle,
            id_key_map: AHashMap::new(),
            key_id_map: AHashMap::new(),
            read_buffer: vec![0u8; 4096].into_boxed_slice(),
            write_queue: VecDeque::new(),
        }
    }

    fn send_empty_result(&mut self, req: Arc<CacheQueryKey>, expired: bool) {
        let result = EffectiveCacheData::empty(
            self.config.protective_cache_ttl,
            self.config.cache_vanish_wait,
        );
        self.query_handle.send_rsp_data(req, result, expired);
    }

    fn handle_req(&mut self, req: Arc<CacheQueryKey>) {
        use rmpv::ValueRef;

        if self
            .query_handle
            .should_send_raw_query(Arc::clone(&req), self.config.query_wait_timeout)
        {
            let query_id = Uuid::new_v4();

            let mut map = vec![
                (
                    ValueRef::String("id".into()),
                    ValueRef::Binary(query_id.as_bytes()),
                ),
                (
                    ValueRef::String("user".into()),
                    ValueRef::String(req.user.as_str().into()),
                ),
                (
                    ValueRef::String("host".into()),
                    ValueRef::String(req.host.as_str().into()),
                ),
            ];
            if let Some(client_ip) = &req.client_ip {
                map.push((
                    ValueRef::String("client_ip".into()),
                    ValueRef::String(client_ip.as_str().into()),
                ));
            }
            let mut buf = Vec::new();
            let v = ValueRef::Map(map);
            if rmpv::encode::write_value_ref(&mut buf, &v).is_err() {
                self.send_empty_result(req, false);
            } else {
                self.id_key_map.insert(query_id, Arc::clone(&req));
                self.key_id_map.insert(Arc::clone(&req), query_id);
                self.write_queue.push_back((req, buf));
            }
        }
    }

    fn parse_rsp(
        map: Vec<(rmpv::ValueRef, rmpv::ValueRef)>,
    ) -> anyhow::Result<(Uuid, Vec<WeightedValue<MetricsName>>, u32)> {
        use anyhow::Context;
        use rmpv::ValueRef;

        const KEY_ID: &str = "id";

        let mut id: Option<Uuid> = None;
        let mut nodes = Vec::<WeightedValue<MetricsName>>::new();
        let mut ttl: u32 = 0;

        for (k, v) in map {
            let key = g3_msgpack::value::as_string(&k)?;
            match g3_msgpack::key::normalize(key.as_str()).as_str() {
                KEY_ID => {
                    let v = g3_msgpack::value::as_uuid(&v)?;
                    id = Some(v);
                }
                "nodes" => match v {
                    ValueRef::String(_) => {
                        let item = g3_msgpack::value::as_weighted_metrics_name(&v)
                            .context(format!("invalid weighted name string value for key {key}"))?;
                        nodes.push(item);
                    }
                    ValueRef::Array(seq) => {
                        for (i, v) in seq.iter().enumerate() {
                            let item = g3_msgpack::value::as_weighted_metrics_name(v).context(
                                format!("invalid weighted name string value for {key}#{i}"),
                            )?;
                            nodes.push(item);
                        }
                    }
                    _ => return Err(anyhow!("invalid value type for key {key}")),
                },
                "ttl" => {
                    ttl = g3_msgpack::value::as_u32(&v)
                        .context(format!("invalid u32 value for key {key}"))?;
                }
                _ => return Err(anyhow!("invalid key {key}")),
            }
        }

        nodes.reverse(); // reverse as we push to the back
        match id {
            Some(id) => Ok((id, nodes, ttl)),
            None => Err(anyhow!("no required key '{KEY_ID}' found")),
        }
    }

    fn handle_rsp(&mut self, len: usize) {
        use rmpv::ValueRef;

        let mut buf = &self.read_buffer[..len];
        if let Ok(ValueRef::Map(map)) = rmpv::decode::read_value_ref(&mut buf) {
            if let Ok((id, nodes, mut ttl)) = Self::parse_rsp(map) {
                if let Some(req) = self.id_key_map.remove(&id) {
                    if ttl == 0 {
                        ttl = self.config.protective_cache_ttl;
                    } else if ttl > self.config.maximum_cache_ttl {
                        ttl = self.config.maximum_cache_ttl;
                    }

                    let mut builder = SelectiveVecBuilder::new();
                    for node in nodes {
                        builder.insert(node);
                    }
                    let nodes = builder.build().unwrap_or_else(|_| SelectiveVec::empty());

                    self.key_id_map.remove(&req);
                    let result = EffectiveCacheData::new(nodes, ttl, self.config.cache_vanish_wait);
                    self.query_handle.send_rsp_data(req, result, false);
                }
            };
        }
    }

    fn handle_query_failed(&mut self, req: Arc<CacheQueryKey>, expired: bool) {
        if let Some(id) = self.key_id_map.remove(&req) {
            self.id_key_map.remove(&id);
        }
        self.send_empty_result(req, expired);
    }

    fn poll_loop(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        loop {
            // handle rsp
            let mut buf = ReadBuf::new(&mut self.read_buffer);
            match self.socket.poll_recv(cx, &mut buf) {
                Poll::Pending => {}
                Poll::Ready(Err(e)) => return Poll::Ready(Err(e)), // TODO handle error
                Poll::Ready(Ok(_)) => {
                    let len = buf.filled().len();
                    if len > 0 {
                        self.handle_rsp(len);
                    }
                }
            }

            // send req from write queue
            while let Some((req, v)) = self.write_queue.pop_front() {
                match self.socket.poll_send(cx, v.as_slice()) {
                    Poll::Pending => {
                        self.write_queue.push_front((req, v));
                        break;
                    }
                    Poll::Ready(Ok(_)) => {}
                    Poll::Ready(Err(_)) => self.handle_query_failed(req, false),
                }
            }

            // handle timeout
            loop {
                match self.query_handle.poll_query_expired(cx) {
                    Poll::Pending => break,
                    Poll::Ready(None) => break,
                    Poll::Ready(Some(req)) => self.handle_query_failed(req, true),
                }
            }

            // handle req
            match self.query_handle.poll_recv_req(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => return Poll::Ready(Ok(())),
                Poll::Ready(Some(req)) => self.handle_req(req),
            }
        }
    }
}

impl Future for QueryRuntime {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        (*self).poll_loop(cx)
    }
}
