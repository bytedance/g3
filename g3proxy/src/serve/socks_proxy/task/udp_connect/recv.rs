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

use std::future::poll_fn;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::task::{ready, Context, Poll};

use g3_io_ext::{AsyncUdpRecv, UdpCopyClientError, UdpCopyClientRecv};
use g3_socks::v5::UdpInput;
use g3_types::acl::{AclAction, AclNetworkRule};
use g3_types::net::UpstreamAddr;

pub(super) struct Socks5UdpConnectClientRecv<T> {
    inner: T,
    client_addr: SocketAddr,
    upstream: UpstreamAddr,
}

impl<T> Socks5UdpConnectClientRecv<T>
where
    T: AsyncUdpRecv,
{
    pub(super) fn new(inner: T, client: Option<SocketAddr>) -> Self {
        let client_addr =
            client.unwrap_or_else(|| SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0));
        Socks5UdpConnectClientRecv {
            inner,
            client_addr,
            upstream: UpstreamAddr::empty(),
        }
    }

    pub(super) fn inner(&self) -> &T {
        &self.inner
    }

    pub(super) fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    fn poll_recv(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<(usize, usize, UpstreamAddr), UdpCopyClientError>> {
        let nr = ready!(self.inner.poll_recv(cx, buf)).map_err(UdpCopyClientError::RecvFailed)?;

        let (off, upstream) = UdpInput::parse_header(buf)
            .map_err(|e| UdpCopyClientError::InvalidPacket(e.to_string()))?;
        Poll::Ready(Ok((off, nr, upstream)))
    }

    fn poll_recv_first(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
        ingress_net_filter: &Option<Arc<AclNetworkRule>>,
    ) -> Poll<Result<(usize, usize), UdpCopyClientError>> {
        let expected_ip = self.client_addr.ip();
        let expected_port = self.client_addr.port();
        let set_client = expected_ip.is_unspecified() || expected_port == 0;

        let (nr, client_addr) =
            ready!(self.inner.poll_recv_from(cx, buf)).map_err(UdpCopyClientError::RecvFailed)?;

        if set_client {
            if !expected_ip.is_unspecified() && expected_ip != client_addr.ip() {
                return Poll::Ready(Err(UdpCopyClientError::MismatchedClientAddress));
            }
            if expected_port != 0 && expected_port != client_addr.port() {
                // TODO log
            }
        } else if self.client_addr.ne(&client_addr) {
            return Poll::Ready(Err(UdpCopyClientError::MismatchedClientAddress));
        }

        if let Some(ingress_net_filter) = ingress_net_filter {
            let (_, action) = ingress_net_filter.check(client_addr.ip());
            match action {
                AclAction::Permit => {}
                AclAction::PermitAndLog => {
                    // TODO log
                }
                AclAction::Forbid => {
                    return Poll::Ready(Err(UdpCopyClientError::ForbiddenClientAddress));
                }
                AclAction::ForbidAndLog => {
                    // TODO log
                    return Poll::Ready(Err(UdpCopyClientError::ForbiddenClientAddress));
                }
            }
        }

        self.client_addr = client_addr;

        let (off, upstream) = UdpInput::parse_header(buf)
            .map_err(|e| UdpCopyClientError::InvalidPacket(e.to_string()))?;
        self.upstream = upstream;

        Poll::Ready(Ok((off, nr)))
    }

    pub async fn recv_first_packet(
        &mut self,
        buf: &mut [u8],
        ingress_net_filter: &Option<Arc<AclNetworkRule>>,
    ) -> Result<(usize, usize, SocketAddr, UpstreamAddr), UdpCopyClientError> {
        loop {
            // only receive the first valid packet
            match poll_fn(|cx| self.poll_recv_first(cx, buf, ingress_net_filter)).await {
                Ok((off, nr)) => return Ok((off, nr, self.client_addr, self.upstream.clone())),
                Err(UdpCopyClientError::MismatchedClientAddress) => {}
                Err(e) => return Err(e),
            }
        }
    }
}

impl<T> UdpCopyClientRecv for Socks5UdpConnectClientRecv<T>
where
    T: AsyncUdpRecv + Send,
{
    /// reserve some space for offloading header
    fn buf_reserve_length(&self) -> usize {
        256 + 4 + 2
    }

    fn poll_recv_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<(usize, usize), UdpCopyClientError>> {
        let (off, nr, upstream) = ready!(self.poll_recv(cx, buf))?;
        if self.upstream.eq(&upstream) {
            Poll::Ready(Ok((off, nr)))
        } else {
            Poll::Ready(Err(UdpCopyClientError::VaryUpstream))
        }
    }
}
