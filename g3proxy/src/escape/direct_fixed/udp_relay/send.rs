/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::num::NonZero;
use std::sync::Arc;
use std::task::{Context, Poll, ready};

use arcstr::ArcStr;
use lru::LruCache;
use slog::Logger;

#[cfg(any(
    target_os = "linux",
    target_os = "android",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd",
    target_os = "solaris",
))]
use g3_io_ext::UdpRelayPacket;
use g3_io_ext::{AsyncUdpSend, UdpRelayRemoteError, UdpRelayRemoteSend};
use g3_resolver::{ResolveError, ResolveLocalError};
use g3_types::acl::{AclAction, AclNetworkRule};
use g3_types::net::{Host, UpstreamAddr};
use g3_types::resolve::ResolveStrategy;

use super::DirectFixedEscaperStats;
use crate::auth::UserContext;
use crate::resolve::{ArcIntegratedResolverHandle, ArriveFirstResolveJob};

const LRU_CACHE_SIZE: NonZero<usize> = NonZero::new(16).unwrap();

pub(crate) struct DirectUdpRelayRemoteSend<T> {
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
    resolve_retry_domain: Option<ArcStr>,
    resolved_lru: LruCache<ArcStr, IpAddr>,
    logger: Option<Logger>,
}

impl<T> DirectUdpRelayRemoteSend<T> {
    pub(crate) fn new(
        escaper_stats: &Arc<DirectFixedEscaperStats>,
        user_ctx: Option<&UserContext>,
        egress_net_filter: &Arc<AclNetworkRule>,
        resolver_handle: &ArcIntegratedResolverHandle,
        resolve_strategy: ResolveStrategy,
        logger: Option<Logger>,
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
            resolved_lru: LruCache::new(LRU_CACHE_SIZE),
            logger,
        }
    }
}

impl<T> DirectUdpRelayRemoteSend<T>
where
    T: AsyncUdpSend,
{
    pub(crate) fn enable_v4(&mut self, inner: T, bind: SocketAddr) {
        self.inner_v4 = Some(inner);
        self.bind_v4 = bind;
    }

    pub(crate) fn enable_v6(&mut self, inner: T, bind: SocketAddr) {
        self.inner_v6 = Some(inner);
        self.bind_v6 = bind;
    }

    pub(crate) fn usable(&self) -> bool {
        self.inner_v4.is_some() || self.inner_v6.is_some()
    }

    fn poll_send_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
        to: &UpstreamAddr,
    ) -> Poll<Result<usize, UdpRelayRemoteError>> {
        match to.host() {
            Host::Ip(ip) => self.poll_send_ip_packet(cx, buf, SocketAddr::new(*ip, to.port())),
            Host::Domain(domain) => match self.resolved_lru.get(domain) {
                Some(ip) => {
                    let to_addr = SocketAddr::new(*ip, to.port());
                    self.poll_send_ip_packet(cx, buf, to_addr)
                }
                None => {
                    loop {
                        if let Some(mut resolver_job) = self.resolver_job.take() {
                            match resolver_job.poll_best_addr(cx) {
                                Poll::Pending => {
                                    self.resolver_job = Some(resolver_job);
                                    return Poll::Pending;
                                }
                                Poll::Ready(Ok(ip)) => {
                                    self.resolved_lru.push(resolver_job.domain, ip);
                                    return self.poll_send_ip_packet(
                                        cx,
                                        buf,
                                        SocketAddr::new(ip, to.port()),
                                    );
                                }
                                Poll::Ready(Err(e)) => {
                                    if let Some(domain) = self.resolve_retry_domain.take() {
                                        if self.resolver_handle.is_closed() {
                                            match crate::resolve::get_handle(
                                                self.resolver_handle.name(),
                                            ) {
                                                Ok(handle) => {
                                                    self.resolver_handle = handle;
                                                    let resolver_job = ArriveFirstResolveJob::new(
                                                        &self.resolver_handle,
                                                        self.resolve_strategy,
                                                        domain,
                                                    )?;
                                                    self.resolver_job = Some(resolver_job);
                                                    // no retry by leaving resolve_retry_domain to None
                                                }
                                                Err(_) => return Poll::Ready(Err(
                                                    UdpRelayRemoteError::DomainNotResolved(
                                                        ResolveError::FromLocal(
                                                            ResolveLocalError::NoResolverRunning,
                                                        ),
                                                    ),
                                                )),
                                            }
                                        } else {
                                            return Poll::Ready(Err(e.into()));
                                        }
                                    } else {
                                        return Poll::Ready(Err(e.into()));
                                    }
                                }
                            };
                        } else {
                            let resolver_job = ArriveFirstResolveJob::new(
                                &self.resolver_handle,
                                self.resolve_strategy,
                                domain.clone(),
                            )?;
                            self.resolver_job = Some(resolver_job);
                            self.resolve_retry_domain = Some(domain.clone());
                        }
                    }
                }
            },
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
        if let Some(last_ip) = self.checked_egress_ip
            && last_ip == to_ip
        {
            return Ok(());
        }
        let (_, action) = self.egress_net_filter.check(to_ip);
        self.handle_udp_target_ip_acl_action(action, to_addr)?;
        self.checked_egress_ip = Some(to_ip);
        Ok(())
    }

    fn poll_send_ip_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
        to: SocketAddr,
    ) -> Poll<Result<usize, UdpRelayRemoteError>> {
        match to {
            SocketAddr::V4(_) => self.poll_send_v4_packet(cx, buf, to),
            SocketAddr::V6(_) => self.poll_send_v6_packet(cx, buf, to),
        }
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

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "solaris",
    ))]
    fn poll_send_packets(
        inner: &mut T,
        resolved_lru: &mut LruCache<ArcStr, IpAddr>,
        bind_addr: SocketAddr,
        cx: &mut Context<'_>,
        packets: &[UdpRelayPacket],
    ) -> Poll<Result<usize, UdpRelayRemoteError>> {
        use g3_io_sys::udp::SendMsgHdr;
        use std::io::IoSlice;

        let mut msgs: Vec<SendMsgHdr<1>> = packets
            .iter()
            .map(|p| {
                let addr = match p.upstream().host() {
                    Host::Ip(ip) => SocketAddr::new(*ip, p.upstream().port()),
                    Host::Domain(domain) => resolved_lru
                        .get(domain)
                        .map(|ip| SocketAddr::new(*ip, p.upstream().port()))
                        .unwrap(),
                };
                SendMsgHdr::new([IoSlice::new(p.payload())], Some(addr))
            })
            .collect();

        let count = ready!(inner.poll_batch_sendmsg(cx, &mut msgs))
            .map_err(|e| UdpRelayRemoteError::BatchSendFailed(bind_addr, e))?;
        if count == 0 {
            Poll::Ready(Err(UdpRelayRemoteError::BatchSendFailed(
                bind_addr,
                io::Error::new(io::ErrorKind::WriteZero, "write zero packet into sender"),
            )))
        } else {
            Poll::Ready(Ok(count))
        }
    }
}

impl<T> UdpRelayRemoteSend for DirectUdpRelayRemoteSend<T>
where
    T: AsyncUdpSend + Send,
{
    fn error_logger(&self) -> Option<&Logger> {
        self.logger.as_ref()
    }

    fn poll_send_packet(
        &mut self,
        cx: &mut Context<'_>,
        buf: &[u8],
        to: &UpstreamAddr,
    ) -> Poll<Result<usize, UdpRelayRemoteError>> {
        self.poll_send_packet(cx, buf, to)
    }

    #[cfg(any(
        target_os = "linux",
        target_os = "android",
        target_os = "freebsd",
        target_os = "netbsd",
        target_os = "openbsd",
        target_os = "solaris",
    ))]
    fn poll_send_packets(
        &mut self,
        cx: &mut Context<'_>,
        packets: &[UdpRelayPacket],
    ) -> Poll<Result<usize, UdpRelayRemoteError>> {
        let Some(p) = packets.first() else {
            return Poll::Ready(Ok(0));
        };

        let ip = match p.upstream().host() {
            Host::Ip(ip) => *ip,
            Host::Domain(domain) => match self.resolved_lru.get(domain) {
                Some(ip) => *ip,
                None => {
                    let _ = ready!(self.poll_send_packet(cx, p.payload(), p.upstream()))?;
                    return Poll::Ready(Ok(1));
                }
            },
        };

        match ip {
            IpAddr::V4(_) => {
                let mut count = 0;
                for p in packets {
                    let ip = match p.upstream().host() {
                        Host::Ip(IpAddr::V4(v4)) => IpAddr::V4(*v4),
                        Host::Ip(IpAddr::V6(_)) => break,
                        Host::Domain(domain) => match self.resolved_lru.get(domain) {
                            Some(IpAddr::V4(v4)) => IpAddr::V4(*v4),
                            Some(IpAddr::V6(_)) => break,
                            None => break,
                        },
                    };

                    if let Err(e) = self.check_egress_ip(SocketAddr::new(ip, p.upstream().port())) {
                        if count == 0 {
                            return Poll::Ready(Err(e));
                        } else {
                            break;
                        }
                    }

                    count += 1;
                }

                if let Some(inner) = &mut self.inner_v4 {
                    Self::poll_send_packets(
                        inner,
                        &mut self.resolved_lru,
                        self.bind_v4,
                        cx,
                        &packets[0..count],
                    )
                } else {
                    Poll::Ready(Err(UdpRelayRemoteError::AddressNotSupported))
                }
            }
            IpAddr::V6(_) => {
                let mut count = 0;
                for p in packets {
                    let ip = match p.upstream().host() {
                        Host::Ip(IpAddr::V4(_)) => break,
                        Host::Ip(IpAddr::V6(v6)) => IpAddr::V6(*v6),
                        Host::Domain(domain) => match self.resolved_lru.get(domain) {
                            Some(IpAddr::V4(_)) => break,
                            Some(IpAddr::V6(v6)) => IpAddr::V6(*v6),
                            None => break,
                        },
                    };

                    if let Err(e) = self.check_egress_ip(SocketAddr::new(ip, p.upstream().port())) {
                        if count == 0 {
                            return Poll::Ready(Err(e));
                        } else {
                            break;
                        }
                    }

                    count += 1;
                }

                if let Some(inner) = &mut self.inner_v6 {
                    Self::poll_send_packets(
                        inner,
                        &mut self.resolved_lru,
                        self.bind_v6,
                        cx,
                        &packets[0..count],
                    )
                } else {
                    Poll::Ready(Err(UdpRelayRemoteError::AddressNotSupported))
                }
            }
        }
    }
}
