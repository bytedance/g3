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

use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::Arc;
use std::task::{ready, Context, Poll};

use g3_io_ext::{AsyncUdpSend, UdpRelayRemoteError, UdpRelayRemoteSend};
use g3_resolver::{ResolveError, ResolveLocalError};
use g3_types::acl::{AclAction, AclNetworkRule};
use g3_types::net::{Host, UpstreamAddr};
use g3_types::resolve::ResolveStrategy;

use super::DirectFixedEscaperStats;
use crate::auth::UserContext;
use crate::resolve::{ArcIntegratedResolverHandle, ArriveFirstResolveJob};

pub(super) struct DirectUdpRelayRemoteSend<T> {
    escaper_stats: Arc<DirectFixedEscaperStats>,
    user_ctx: Option<UserContext>,
    inner_v4: Option<T>,
    inner_v6: Option<T>,
    bind_v4: SocketAddr,
    bind_v6: SocketAddr,
    egress_net_filter: Arc<AclNetworkRule>,
    checked_egress_ip: Option<IpAddr>,
    resolver_handle: ArcIntegratedResolverHandle,
    resolve_strategy: ResolveStrategy,
    resolver_job: Option<ArriveFirstResolveJob>,
    resolve_retry_domain: Option<String>,
    resolved_port: u16,
    resolved_ip: Option<IpAddr>,
}

impl<T> DirectUdpRelayRemoteSend<T> {
    pub(super) fn new(
        escaper_stats: &Arc<DirectFixedEscaperStats>,
        user_ctx: Option<&UserContext>,
        egress_net_filter: &Arc<AclNetworkRule>,
        resolver_handle: &ArcIntegratedResolverHandle,
        resolve_strategy: ResolveStrategy,
    ) -> Self {
        DirectUdpRelayRemoteSend {
            escaper_stats: Arc::clone(escaper_stats),
            user_ctx: user_ctx.cloned(),
            inner_v4: None,
            inner_v6: None,
            bind_v4: SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
            bind_v6: SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
            egress_net_filter: Arc::clone(egress_net_filter),
            checked_egress_ip: None,
            resolver_handle: Arc::clone(resolver_handle),
            resolve_strategy,
            resolver_job: None,
            resolve_retry_domain: None,
            resolved_port: 0,
            resolved_ip: None,
        }
    }
}

impl<T> DirectUdpRelayRemoteSend<T>
where
    T: AsyncUdpSend,
{
    pub(super) fn enable_v4(&mut self, inner: T, bind: SocketAddr) {
        self.inner_v4 = Some(inner);
        self.bind_v4 = bind;
    }

    pub(super) fn enable_v6(&mut self, inner: T, bind: SocketAddr) {
        self.inner_v6 = Some(inner);
        self.bind_v6 = bind;
    }

    fn poll_send_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
        buf_off: usize,
        buf_len: usize,
        to: &UpstreamAddr,
    ) -> Poll<Result<usize, UdpRelayRemoteError>> {
        if let Some(resolved_ip) = self.resolved_ip.take() {
            let port = self.resolved_port;
            let ret = match resolved_ip {
                IpAddr::V4(_) => self.poll_send_v4_packet(
                    cx,
                    &buf[buf_off..buf_len],
                    SocketAddr::new(resolved_ip, port),
                ),
                IpAddr::V6(_) => self.poll_send_v6_packet(
                    cx,
                    &buf[buf_off..buf_len],
                    SocketAddr::new(resolved_ip, port),
                ),
            };
            if ret.is_pending() {
                self.resolved_ip = Some(resolved_ip);
            }
            return ret;
        }

        if let Some(mut resolver_job) = self.resolver_job.take() {
            return match resolver_job.poll_best_addr(cx) {
                Poll::Pending => {
                    self.resolver_job = Some(resolver_job);
                    Poll::Pending
                }
                Poll::Ready(Ok(ip)) => {
                    self.resolved_ip = Some(ip);
                    self.poll_send_packet(cx, buf, buf_off, buf_len, to)
                }
                Poll::Ready(Err(e)) => {
                    if let Some(domain) = self.resolve_retry_domain.take() {
                        if self.resolver_handle.is_closed() {
                            match crate::resolve::get_handle(self.resolver_handle.name()) {
                                Ok(handle) => {
                                    self.resolver_handle = handle;
                                    let resolver_job = ArriveFirstResolveJob::new(
                                        &self.resolver_handle,
                                        self.resolve_strategy,
                                        &domain,
                                    )?;
                                    self.resolver_job = Some(resolver_job);
                                    // no retry by leaving resolve_retry_domain to None
                                    self.poll_send_packet(cx, buf, buf_off, buf_len, to)
                                }
                                Err(_) => Poll::Ready(Err(UdpRelayRemoteError::DomainNotResolved(
                                    ResolveError::FromLocal(ResolveLocalError::NoResolverRunning),
                                ))),
                            }
                        } else {
                            Poll::Ready(Err(e.into()))
                        }
                    } else {
                        Poll::Ready(Err(e.into()))
                    }
                }
            };
        }

        match to.host() {
            Host::Ip(IpAddr::V4(ip)) => self.poll_send_v4_packet(
                cx,
                &buf[buf_off..buf_len],
                SocketAddr::new(IpAddr::V4(*ip), to.port()),
            ),
            Host::Ip(IpAddr::V6(ip)) => self.poll_send_v6_packet(
                cx,
                &buf[buf_off..buf_len],
                SocketAddr::new(IpAddr::V6(*ip), to.port()),
            ),
            Host::Domain(domain) => {
                self.resolved_port = to.port();
                let resolver_job = ArriveFirstResolveJob::new(
                    &self.resolver_handle,
                    self.resolve_strategy,
                    domain,
                )?;
                self.resolver_job = Some(resolver_job);
                self.resolve_retry_domain = Some(domain.to_string());
                self.poll_send_packet(cx, buf, buf_off, buf_len, to)
            }
        }
    }

    fn handle_udp_target_ip_acl_action(
        &self,
        action: AclAction,
        to_addr: SocketAddr,
    ) -> Result<(), UdpRelayRemoteError> {
        let forbid = match action {
            AclAction::Permit => false,
            AclAction::PermitAndLog => {
                // TODO log
                false
            }
            AclAction::Forbid => true,
            AclAction::ForbidAndLog => {
                // TODO log
                true
            }
        };
        if forbid {
            self.escaper_stats.forbidden.add_ip_blocked();
            if let Some(user_ctx) = &self.user_ctx {
                user_ctx.add_ip_blocked();
            }
            Err(UdpRelayRemoteError::ForbiddenTargetIpAddress(to_addr))
        } else {
            Ok(())
        }
    }

    fn check_egress_ip(&mut self, to_addr: SocketAddr) -> Result<(), UdpRelayRemoteError> {
        let to_ip = to_addr.ip();
        if let Some(last_ip) = self.checked_egress_ip {
            if last_ip == to_ip {
                return Ok(());
            }
        }
        let (_, action) = self.egress_net_filter.check(to_ip);
        self.handle_udp_target_ip_acl_action(action, to_addr)?;
        self.checked_egress_ip = Some(to_ip);
        Ok(())
    }

    fn poll_send_v4_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
        to: SocketAddr,
    ) -> Poll<Result<usize, UdpRelayRemoteError>> {
        self.check_egress_ip(to)?;
        if let Some(inner) = &mut self.inner_v4 {
            let nw = ready!(inner.poll_send_to(cx, buf, to))
                .map_err(|e| UdpRelayRemoteError::SendFailed(self.bind_v4, to, e))?;
            if nw == 0 {
                Poll::Ready(Err(UdpRelayRemoteError::SendFailed(
                    self.bind_v4,
                    to,
                    io::Error::new(io::ErrorKind::WriteZero, "write zero byte into sender"),
                )))
            } else {
                Poll::Ready(Ok(nw))
            }
        } else {
            Poll::Ready(Err(UdpRelayRemoteError::AddressNotSupported))
        }
    }

    fn poll_send_v6_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
        to: SocketAddr,
    ) -> Poll<Result<usize, UdpRelayRemoteError>> {
        self.check_egress_ip(to)?;
        if let Some(inner) = &mut self.inner_v6 {
            let nw = ready!(inner.poll_send_to(cx, buf, to))
                .map_err(|e| UdpRelayRemoteError::SendFailed(self.bind_v6, to, e))?;
            if nw == 0 {
                Poll::Ready(Err(UdpRelayRemoteError::SendFailed(
                    self.bind_v6,
                    to,
                    io::Error::new(io::ErrorKind::WriteZero, "write zero byte into sender"),
                )))
            } else {
                Poll::Ready(Ok(nw))
            }
        } else {
            Poll::Ready(Err(UdpRelayRemoteError::AddressNotSupported))
        }
    }
}

impl<T> UdpRelayRemoteSend for DirectUdpRelayRemoteSend<T>
where
    T: AsyncUdpSend + Send,
{
    fn buf_reserve_length(&self) -> usize {
        0
    }

    fn poll_send_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &mut [u8],
        buf_off: usize,
        buf_len: usize,
        to: &UpstreamAddr,
    ) -> Poll<Result<usize, UdpRelayRemoteError>> {
        self.poll_send_packet(cx, buf, buf_off, buf_len, to)
    }
}
