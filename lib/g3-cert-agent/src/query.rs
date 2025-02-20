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
use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;

use anyhow::anyhow;
use log::{debug, warn};
use tokio::io::ReadBuf;
use tokio::net::UdpSocket;

use g3_io_ext::{EffectiveCacheData, EffectiveQueryHandle};

use super::{CacheQueryKey, CertAgentConfig, FakeCertPair, Response};

pub(super) struct QueryRuntime {
    socket: UdpSocket,
    query_handle: EffectiveQueryHandle<CacheQueryKey, FakeCertPair>,
    read_buffer: Box<[u8]>,
    write_queue: VecDeque<(Arc<CacheQueryKey>, Vec<u8>)>,
    protective_ttl: u32,
    maximum_ttl: u32,
    vanish_wait: Duration,
    query_wait: Duration,
}

impl QueryRuntime {
    pub(super) fn new(
        config: &CertAgentConfig,
        socket: UdpSocket,
        query_handle: EffectiveQueryHandle<CacheQueryKey, FakeCertPair>,
    ) -> Self {
        QueryRuntime {
            socket,
            query_handle,
            read_buffer: vec![0u8; 16384].into_boxed_slice(),
            write_queue: VecDeque::new(),
            protective_ttl: config.protective_cache_ttl,
            maximum_ttl: config.maximum_cache_ttl,
            vanish_wait: config.cache_vanish_wait,
            query_wait: config.query_wait_timeout,
        }
    }

    fn send_empty_result(&mut self, req: Arc<CacheQueryKey>, expired: bool) {
        let result = EffectiveCacheData::empty(self.protective_ttl, self.vanish_wait);
        self.query_handle.send_rsp_data(req, result, expired);
    }

    fn handle_req(&mut self, req: Arc<CacheQueryKey>) {
        if self
            .query_handle
            .should_send_raw_query(req.clone(), self.query_wait)
        {
            match req.encode() {
                Ok(buf) => self.write_queue.push_back((req, buf)),
                Err(e) => {
                    warn!("failed to encode cert generate request ro msgpack: {e}");
                    self.send_empty_result(req, false)
                }
            }
        }
    }

    fn handle_rsp(&mut self, len: usize) {
        let mut buf = &self.read_buffer[..len];
        match rmpv::decode::read_value_ref(&mut buf)
            .map_err(|e| anyhow!("invalid msgpack response data: {e}"))
            .and_then(|v| Response::parse(v, self.protective_ttl))
            .and_then(|r| r.into_parts())
        {
            Ok((req_key, pair, mut ttl)) => {
                if ttl == 0 {
                    ttl = self.protective_ttl;
                } else if ttl > self.maximum_ttl {
                    ttl = self.maximum_ttl;
                }

                let result = EffectiveCacheData::new(pair, ttl, self.vanish_wait);
                self.query_handle
                    .send_rsp_data(Arc::new(req_key), result, false);
            }
            Err(e) => {
                warn!("parse cert generator rsp error: {e:?}");
            }
        }
    }

    fn poll_loop(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        loop {
            // handle rsp
            let mut buf = ReadBuf::new(&mut self.read_buffer);
            match self.socket.poll_recv(cx, &mut buf) {
                Poll::Pending => {}
                Poll::Ready(Err(e)) => {
                    warn!("socket recv error: {e:?}");
                    return Poll::Ready(Err(e));
                }
                Poll::Ready(Ok(_)) => {
                    let len = buf.filled().len();
                    if len > 0 {
                        self.handle_rsp(len);
                    }
                    continue;
                }
            }

            // send req from write queue
            while let Some((req_key, v)) = self.write_queue.pop_front() {
                match self.socket.poll_send(cx, v.as_slice()) {
                    Poll::Pending => {
                        self.write_queue.push_front((req_key, v));
                        break;
                    }
                    Poll::Ready(Ok(_)) => {}
                    Poll::Ready(Err(e)) => {
                        debug!("failed to send out cert generate request: {e}");
                        self.send_empty_result(req_key, false);
                    }
                }
            }

            // handle timeout
            loop {
                match self.query_handle.poll_query_expired(cx) {
                    Poll::Pending => break,
                    Poll::Ready(None) => break,
                    Poll::Ready(Some(t)) => {
                        debug!("cert generation query timeout for {}", t.index.host);
                        self.send_empty_result(t, true)
                    }
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
