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
use std::time::Duration;

use anyhow::anyhow;
use log::warn;
use rustls::{Certificate, PrivateKey};
use tokio::io::ReadBuf;
use tokio::net::UdpSocket;

use g3_io_ext::{EffectiveCacheData, EffectiveQueryHandle};

use super::{CacheQueryKey, CertAgentConfig};

pub(super) struct QueryRuntime {
    socket: UdpSocket,
    query_handle: EffectiveQueryHandle<CacheQueryKey, (Vec<Certificate>, PrivateKey)>,
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
        query_handle: EffectiveQueryHandle<CacheQueryKey, (Vec<Certificate>, PrivateKey)>,
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
        use rmpv::ValueRef;

        if self
            .query_handle
            .should_send_raw_query(req.clone(), self.query_wait)
        {
            let map = vec![(
                ValueRef::String("host".into()),
                ValueRef::String(req.host.as_str().into()),
            )];
            let mut buf = Vec::with_capacity(32);
            let v = ValueRef::Map(map);
            if rmpv::encode::write_value_ref(&mut buf, &v).is_err() {
                self.send_empty_result(req, false);
                return;
            }
            self.write_queue.push_back((req, buf));
        }
    }

    fn parse_rsp(
        map: Vec<(rmpv::ValueRef, rmpv::ValueRef)>,
    ) -> anyhow::Result<(Arc<CacheQueryKey>, Vec<Certificate>, PrivateKey, u32)> {
        use anyhow::Context;

        let mut host = String::new();
        let mut cert = Vec::new();
        let mut pkey = PrivateKey(Vec::new());
        let mut ttl: u32 = 0;

        for (k, v) in map {
            let key = g3_msgpack::value::as_string(&k)?;
            match g3_msgpack::key::normalize(key.as_str()).as_str() {
                "host" => {
                    host = g3_msgpack::value::as_string(&v)
                        .context(format!("invalid string value for key {key}"))?;
                }
                "cert" => {
                    cert = g3_msgpack::value::as_certificates(&v)
                        .context(format!("invalid tls certificate value for key {key}"))?;
                }
                "key" => {
                    pkey = g3_msgpack::value::as_private_key(&v)
                        .context(format!("invalid tls private key value for key {key}"))?;
                }
                "ttl" => {
                    ttl = g3_msgpack::value::as_u32(&v)
                        .context(format!("invalid u32 value for key {key}"))?;
                }
                _ => return Err(anyhow!("invalid key {key}")),
            }
        }

        if host.is_empty() {
            return Err(anyhow!("no required host key found"));
        }
        if cert.is_empty() {
            return Err(anyhow!("no required cert key found"));
        }
        if pkey.0.is_empty() {
            return Err(anyhow!("no required pkey key found"));
        }

        Ok((Arc::new(CacheQueryKey { host }), cert, pkey, ttl))
    }

    fn handle_rsp(&mut self, len: usize) {
        use rmpv::ValueRef;

        let mut buf = &self.read_buffer[..len];
        if let Ok(ValueRef::Map(map)) = rmpv::decode::read_value_ref(&mut buf) {
            match Self::parse_rsp(map) {
                Ok((req_key, cert, key, mut ttl)) => {
                    if ttl == 0 {
                        ttl = self.protective_ttl;
                    } else if ttl > self.maximum_ttl {
                        ttl = self.maximum_ttl;
                    }

                    let result = EffectiveCacheData::new((cert, key), ttl, self.vanish_wait);
                    self.query_handle.send_rsp_data(req_key, result, false);
                }
                Err(e) => {
                    warn!("parse cert generator rsp error: {e:?}");
                }
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
                    Poll::Ready(Err(_)) => self.send_empty_result(req_key, false),
                }
            }

            // handle timeout
            loop {
                match self.query_handle.poll_query_expired(cx) {
                    Poll::Pending => break,
                    Poll::Ready(None) => break,
                    Poll::Ready(Some(t)) => self.send_empty_result(t, true),
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
