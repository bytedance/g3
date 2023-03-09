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

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::task::{ready, Context, Poll};

use g3_io_ext::{AsyncUdpRecv, UdpRelayRemoteError, UdpRelayRemoteRecv};
use g3_types::net::UpstreamAddr;

pub(super) struct DirectUdpRelayRemoteRecv<T> {
    inner_v4: Option<T>,
    inner_v6: Option<T>,
    bind_v4: SocketAddr,
    bind_v6: SocketAddr,
}

impl<T> DirectUdpRelayRemoteRecv<T> {
    pub(super) fn new() -> Self {
        DirectUdpRelayRemoteRecv {
            inner_v4: None,
            inner_v6: None,
            bind_v4: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
            bind_v6: SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
        }
    }
}

impl<T> DirectUdpRelayRemoteRecv<T>
where
    T: AsyncUdpRecv,
{
    pub(super) fn enable_v4(&mut self, inner: T, bind: SocketAddr) {
        self.inner_v4 = Some(inner);
        self.bind_v4 = bind;
    }

    pub(super) fn enable_v6(&mut self, inner: T, bind: SocketAddr) {
        self.inner_v6 = Some(inner);
        self.bind_v6 = bind;
    }

    fn poll_recv_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<(usize, usize, SocketAddr), UdpRelayRemoteError>> {
        match (&mut self.inner_v4, &mut self.inner_v6) {
            (Some(inner_v4), Some(inner_v6)) => {
                let ret = match inner_v4.poll_recv_from(cx, buf) {
                    Poll::Ready(t) => {
                        let (nr, addr) =
                            t.map_err(|e| UdpRelayRemoteError::RecvFailed(self.bind_v4, e))?;
                        Ok((0, nr, addr))
                    }
                    Poll::Pending => {
                        let (nr, addr) = ready!(inner_v6.poll_recv_from(cx, buf))
                            .map_err(|e| UdpRelayRemoteError::RecvFailed(self.bind_v6, e))?;
                        Ok((0, nr, addr))
                    }
                };
                Poll::Ready(ret)
            }
            (Some(inner_v4), None) => {
                let (nr, addr) = ready!(inner_v4.poll_recv_from(cx, buf))
                    .map_err(|e| UdpRelayRemoteError::RecvFailed(self.bind_v4, e))?;
                Poll::Ready(Ok((0, nr, addr)))
            }
            (None, Some(inner_v6)) => {
                let (nr, addr) = ready!(inner_v6.poll_recv_from(cx, buf))
                    .map_err(|e| UdpRelayRemoteError::RecvFailed(self.bind_v6, e))?;
                Poll::Ready(Ok((0, nr, addr)))
            }
            (None, None) => Poll::Ready(Err(UdpRelayRemoteError::NoListenSocket)),
        }
    }
}

impl<T> UdpRelayRemoteRecv for DirectUdpRelayRemoteRecv<T>
where
    T: AsyncUdpRecv + Send,
{
    fn buf_reserve_length(&self) -> usize {
        0
    }

    fn poll_recv_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<(usize, usize, UpstreamAddr), UdpRelayRemoteError>> {
        let (off, nr, addr) = ready!(self.poll_recv_packet(cx, buf))?;
        Poll::Ready(Ok((
            off,
            nr,
            UpstreamAddr::from_ip_and_port(addr.ip(), addr.port()),
        )))
    }
}
