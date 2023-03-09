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

use g3_io_ext::{AsyncUdpRecv, UdpRelayClientError, UdpRelayClientRecv};
use g3_socks::v5::UdpInput;
use g3_types::acl::{AclAction, AclNetworkRule};
use g3_types::net::UpstreamAddr;

use super::CommonTaskContext;
use crate::auth::UserContext;

pub(super) struct Socks5UdpAssociateClientRecv<T> {
    inner: T,
    client_addr: SocketAddr,
    ctx: Arc<CommonTaskContext>,
    user_ctx: Option<UserContext>,
}

impl<T> Socks5UdpAssociateClientRecv<T>
where
    T: AsyncUdpRecv,
{
    pub(super) fn new(
        inner: T,
        client: Option<SocketAddr>,
        ctx: &Arc<CommonTaskContext>,
        user_ctx: Option<&UserContext>,
    ) -> Self {
        let client_addr =
            client.unwrap_or_else(|| SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0));
        Socks5UdpAssociateClientRecv {
            inner,
            client_addr,
            ctx: Arc::clone(ctx),
            user_ctx: user_ctx.cloned(),
        }
    }

    pub(super) fn inner(&self) -> &T {
        &self.inner
    }

    pub(super) fn inner_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    fn handle_user_upstream_acl_action(
        &self,
        action: AclAction,
    ) -> Result<(), UdpRelayClientError> {
        let forbid = match action {
            AclAction::Permit => false,
            AclAction::PermitAndLog => {
                // TODO log permit
                false
            }
            AclAction::Forbid => true,
            AclAction::ForbidAndLog => {
                // TODO log forbid
                true
            }
        };
        if forbid {
            Err(UdpRelayClientError::ForbiddenTargetAddress)
        } else {
            Ok(())
        }
    }

    fn handle_server_upstream_acl_action(
        &self,
        action: AclAction,
    ) -> Result<(), UdpRelayClientError> {
        let forbid = match action {
            AclAction::Permit => false,
            AclAction::PermitAndLog => {
                // TODO log permit
                false
            }
            AclAction::Forbid => true,
            AclAction::ForbidAndLog => {
                // TODO log forbid
                true
            }
        };
        if forbid {
            self.ctx.server_stats.forbidden.add_dest_denied();
            if let Some(user_ctx) = &self.user_ctx {
                // also add to user level forbidden stats
                user_ctx.add_dest_denied();
            }

            Err(UdpRelayClientError::ForbiddenTargetAddress)
        } else {
            Ok(())
        }
    }

    fn check_upstream(&self, upstream: &UpstreamAddr) -> Result<(), UdpRelayClientError> {
        if let Some(user_ctx) = &self.user_ctx {
            let action = user_ctx.check_upstream(upstream);
            self.handle_user_upstream_acl_action(action)?;
        }

        let action = self.ctx.check_upstream(upstream);
        self.handle_server_upstream_acl_action(action)?;

        Ok(())
    }

    fn poll_recv(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
    ) -> Poll<Result<(usize, usize, UpstreamAddr), UdpRelayClientError>> {
        let nr = ready!(self.inner.poll_recv(cx, buf)).map_err(UdpRelayClientError::RecvFailed)?;

        let (off, upstream) = UdpInput::parse_header(buf)
            .map_err(|e| UdpRelayClientError::InvalidPacket(e.to_string()))?;
        self.check_upstream(&upstream)?;
        Poll::Ready(Ok((off, nr, upstream)))
    }

    fn poll_recv_first(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
        ingress_net_filter: &Option<Arc<AclNetworkRule>>,
        initial_peer: &mut UpstreamAddr,
    ) -> Poll<Result<(usize, usize), UdpRelayClientError>> {
        let expected_ip = self.client_addr.ip();
        let expected_port = self.client_addr.port();
        let set_client = expected_ip.is_unspecified() || expected_port == 0;

        let (nr, client_addr) =
            ready!(self.inner.poll_recv_from(cx, buf)).map_err(UdpRelayClientError::RecvFailed)?;

        if set_client {
            if !expected_ip.is_unspecified() && expected_ip != client_addr.ip() {
                return Poll::Ready(Err(UdpRelayClientError::MismatchedClientAddress));
            }
            if expected_port != 0 && expected_port != client_addr.port() {
                // TODO log
            }
        } else if self.client_addr.ne(&client_addr) {
            return Poll::Ready(Err(UdpRelayClientError::MismatchedClientAddress));
        }

        if let Some(ingress_net_filter) = ingress_net_filter {
            let (_, action) = ingress_net_filter.check(client_addr.ip());
            match action {
                AclAction::Permit => {}
                AclAction::PermitAndLog => {
                    // TODO log
                }
                AclAction::Forbid => {
                    return Poll::Ready(Err(UdpRelayClientError::ForbiddenClientAddress));
                }
                AclAction::ForbidAndLog => {
                    // TODO log
                    return Poll::Ready(Err(UdpRelayClientError::ForbiddenClientAddress));
                }
            }
        }

        self.client_addr = client_addr;

        let (off, upstream) = UdpInput::parse_header(buf)
            .map_err(|e| UdpRelayClientError::InvalidPacket(e.to_string()))?;
        *initial_peer = upstream;
        self.check_upstream(initial_peer)?;
        Poll::Ready(Ok((off, nr)))
    }

    pub async fn recv_first_packet(
        &mut self,
        buf: &mut [u8],
        ingress_net_filter: &Option<Arc<AclNetworkRule>>,
        initial_peer: &mut UpstreamAddr,
    ) -> Result<(usize, usize, SocketAddr), UdpRelayClientError> {
        loop {
            // only receive the first valid packet
            match poll_fn(|cx| self.poll_recv_first(cx, buf, ingress_net_filter, initial_peer))
                .await
            {
                Ok((off, nr)) => return Ok((off, nr, self.client_addr)),
                Err(UdpRelayClientError::MismatchedClientAddress) => {}
                Err(e) => return Err(e),
            }
        }
    }
}

impl<T> UdpRelayClientRecv for Socks5UdpAssociateClientRecv<T>
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
    ) -> Poll<Result<(usize, usize, UpstreamAddr), UdpRelayClientError>> {
        self.poll_recv(cx, buf)
    }
}
