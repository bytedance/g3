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

use std::collections::VecDeque;
use std::io;
use std::net::IpAddr;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use anyhow::anyhow;
use log::warn;
use tokio::io::ReadBuf;
use tokio::net::UdpSocket;

use super::{
    IpLocateServiceConfig, IpLocationCacheResponse, IpLocationQueryHandle, Request, Response,
};

pub(crate) struct IpLocationQueryRuntime {
    socket: UdpSocket,
    query_handle: IpLocationQueryHandle,
    read_buffer: Box<[u8]>,
    write_queue: VecDeque<(IpAddr, Vec<u8>)>,
    default_expire_ttl: u32,
    maximum_expire_ttl: u32,
    query_wait: Duration,
}

impl IpLocationQueryRuntime {
    pub(crate) fn new(
        config: &IpLocateServiceConfig,
        socket: UdpSocket,
        query_handle: IpLocationQueryHandle,
    ) -> Self {
        IpLocationQueryRuntime {
            socket,
            query_handle,
            read_buffer: vec![0u8; 16384].into_boxed_slice(),
            write_queue: VecDeque::new(),
            default_expire_ttl: config.default_expire_ttl,
            maximum_expire_ttl: config.maximum_expire_ttl,
            query_wait: config.query_wait_timeout,
        }
    }

    fn send_empty_result(&mut self, ip: IpAddr, ttl: u32, expired: bool) {
        let result = IpLocationCacheResponse::empty(ttl);
        self.query_handle.send_rsp_data(Some(ip), result, expired);
    }

    fn send_expire_ttl(&mut self, ttl: u32) {
        let result = IpLocationCacheResponse::empty(ttl);
        self.query_handle.send_rsp_data(None, result, false);
    }

    fn handle_req(&mut self, ip: IpAddr) {
        if self.query_handle.should_send_raw_query(ip, self.query_wait) {
            match Request::encode_new(ip) {
                Ok(buf) => self.write_queue.push_back((ip, buf)),
                Err(_) => self.send_empty_result(ip, self.default_expire_ttl, false),
            }
        }
    }

    fn handle_rsp(&mut self, len: usize) {
        let mut buf = &self.read_buffer[..len];
        match rmpv::decode::read_value_ref(&mut buf)
            .map_err(|e| anyhow!("invalid msgpack response data: {e}"))
            .and_then(|v| Response::parse(v))
            .map(|r| r.into_parts())
        {
            Ok((ip, location, ttl)) => {
                let ttl = ttl
                    .unwrap_or(self.default_expire_ttl)
                    .min(self.maximum_expire_ttl);

                if let Some(location) = location {
                    let result = IpLocationCacheResponse::new(location, ttl);
                    self.query_handle.send_rsp_data(ip, result, false);
                } else if let Some(ip) = ip {
                    self.send_empty_result(ip, ttl, false);
                } else {
                    self.send_expire_ttl(ttl);
                }
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
            while let Some((ip, buf)) = self.write_queue.pop_front() {
                match self.socket.poll_send(cx, &buf) {
                    Poll::Pending => {
                        self.write_queue.push_front((ip, buf));
                        break;
                    }
                    Poll::Ready(Ok(_)) => {}
                    Poll::Ready(Err(_)) => {
                        self.send_empty_result(ip, self.default_expire_ttl, false)
                    }
                }
            }

            // handle timeout
            loop {
                match self.query_handle.poll_query_expired(cx) {
                    Poll::Pending => break,
                    Poll::Ready(None) => break,
                    Poll::Ready(Some(ip)) => {
                        self.send_empty_result(ip, self.default_expire_ttl, true)
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

impl Future for IpLocationQueryRuntime {
    type Output = io::Result<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        (*self).poll_loop(cx)
    }
}
