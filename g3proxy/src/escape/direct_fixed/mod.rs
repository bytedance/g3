/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use slog::Logger;
use tokio::io::{AsyncWrite, AsyncWriteExt};

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_resolver::ResolveError;
use g3_socket::BindAddr;
use g3_socket::util::AddressFamily;
use g3_types::acl::AclNetworkRule;
use g3_types::metrics::NodeName;
use g3_types::net::{Host, ProxyProtocolEncoder, ProxyProtocolVersion, UpstreamAddr};
use g3_types::resolve::{ResolveRedirection, ResolveStrategy};

use super::{
    ArcEscaper, ArcEscaperStats, EgressPathSelection, Escaper, EscaperInternal, EscaperRegistry,
    EscaperStats,
};
use crate::audit::AuditContext;
use crate::auth::UserUpstreamTrafficStats;
use crate::config::escaper::direct_fixed::DirectFixedEscaperConfig;
use crate::config::escaper::{AnyEscaperConfig, EscaperConfig};
use crate::module::ftp_over_http::{
    ArcFtpTaskRemoteControlStats, ArcFtpTaskRemoteTransferStats, BoxFtpConnectContext,
    BoxFtpRemoteConnection, DirectFtpConnectContext,
};
use crate::module::http_forward::{
    ArcHttpForwardTaskRemoteStats, BoxHttpForwardConnection, BoxHttpForwardContext,
    DirectHttpForwardContext,
};
use crate::module::tcp_connect::{
    TcpConnectError, TcpConnectResult, TcpConnectTaskConf, TcpConnectTaskNotes, TlsConnectTaskConf,
};
use crate::module::udp_connect::{
    ArcUdpConnectTaskRemoteStats, UdpConnectResult, UdpConnectTaskConf, UdpConnectTaskNotes,
};
use crate::module::udp_relay::{
    ArcUdpRelayTaskRemoteStats, UdpRelaySetupResult, UdpRelayTaskConf, UdpRelayTaskNotes,
};
use crate::resolve::{ArcIntegratedResolverHandle, HappyEyeballsResolveJob};
use crate::serve::ServerTaskNotes;

mod stats;
pub(crate) use stats::DirectFixedEscaperStats;

mod ftp_connect;
pub(crate) mod http_forward;
pub(crate) mod tcp_connect;
mod tls_connect;
pub(crate) mod udp_connect;
pub(crate) mod udp_relay;

pub(super) struct DirectFixedEscaper {
    config: Arc<DirectFixedEscaperConfig>,
    stats: Arc<DirectFixedEscaperStats>,
    resolver_handle: ArcIntegratedResolverHandle,
    egress_net_filter: Arc<AclNetworkRule>,
    resolve_redirection: Option<ResolveRedirection>,
    escape_logger: Option<Logger>,
}

impl DirectFixedEscaper {
    fn new_obj(
        config: DirectFixedEscaperConfig,
        stats: Arc<DirectFixedEscaperStats>,
    ) -> anyhow::Result<ArcEscaper> {
        let resolver_handle = crate::resolve::get_handle(config.resolver())?;
        let egress_net_filter = Arc::new(config.egress_net_filter.build());

        let resolve_redirection = config
            .resolve_redirection
            .as_ref()
            .map(|builder| builder.build());

        let escape_logger = config.get_escape_logger();

        stats.set_extra_tags(config.extra_metrics_tags.clone());

        let escaper = DirectFixedEscaper {
            config: Arc::new(config),
            stats,
            resolver_handle,
            egress_net_filter,
            resolve_redirection,
            escape_logger,
        };

        Ok(Arc::new(escaper))
    }

    pub(super) fn prepare_initial(config: DirectFixedEscaperConfig) -> anyhow::Result<ArcEscaper> {
        let stats = Arc::new(DirectFixedEscaperStats::new(config.name()));
        DirectFixedEscaper::new_obj(config, stats)
    }

    fn prepare_reload(
        config: AnyEscaperConfig,
        stats: Arc<DirectFixedEscaperStats>,
    ) -> anyhow::Result<ArcEscaper> {
        if let AnyEscaperConfig::DirectFixed(config) = config {
            DirectFixedEscaper::new_obj(config, stats)
        } else {
            Err(anyhow!("invalid escaper config type"))
        }
    }

    fn get_bind_random(
        &self,
        family: AddressFamily,
        path_selection: Option<&EgressPathSelection>,
    ) -> BindAddr {
        let vec = match family {
            AddressFamily::Ipv4 => &self.config.bind4,
            AddressFamily::Ipv6 => &self.config.bind6,
        };
        match vec.len() {
            #[cfg(any(
                target_os = "linux",
                target_os = "android",
                target_os = "macos",
                target_os = "illumos",
                target_os = "solaris"
            ))]
            0 => self
                .config
                .bind_interface
                .map(BindAddr::Interface)
                .unwrap_or_default(),
            #[cfg(not(any(
                target_os = "linux",
                target_os = "android",
                target_os = "macos",
                target_os = "illumos",
                target_os = "solaris"
            )))]
            0 => BindAddr::None,
            1 => BindAddr::Ip(vec[0]),
            n => {
                if self.config.enable_path_selection {
                    if let Some(path_selection) = path_selection {
                        if let Some(i) = path_selection.select_by_index(n) {
                            return BindAddr::Ip(vec[i]);
                        }
                    }
                }

                fastrand::choice(vec).map(|ip| BindAddr::Ip(*ip)).unwrap()
            }
        }
    }

    fn get_resolve_strategy(&self, task_notes: &ServerTaskNotes) -> ResolveStrategy {
        if let Some(user_ctx) = task_notes.user_ctx() {
            if let Some(rs) = user_ctx.resolve_strategy() {
                self.config.resolve_strategy.adjust_to(rs)
            } else {
                self.config.resolve_strategy
            }
        } else {
            self.config.resolve_strategy
        }
    }

    fn resolve_happy(
        &self,
        domain: Arc<str>,
        strategy: ResolveStrategy,
        task_notes: &ServerTaskNotes,
    ) -> Result<HappyEyeballsResolveJob, ResolveError> {
        if let Some(user_ctx) = task_notes.user_ctx() {
            if let Some(redirect) = user_ctx.user().resolve_redirection() {
                if let Some(v) = redirect.query_value(&domain) {
                    return HappyEyeballsResolveJob::new_redirected(
                        strategy,
                        &self.resolver_handle,
                        v,
                    );
                }
            }
        }

        if let Some(redirect) = &self.resolve_redirection {
            if let Some(v) = redirect.query_value(&domain) {
                return HappyEyeballsResolveJob::new_redirected(strategy, &self.resolver_handle, v);
            }
        }

        HappyEyeballsResolveJob::new_dyn(strategy, &self.resolver_handle, domain)
    }

    async fn resolve_best(
        &self,
        domain: Arc<str>,
        strategy: ResolveStrategy,
    ) -> Result<IpAddr, ResolveError> {
        let mut resolver_job =
            HappyEyeballsResolveJob::new_dyn(strategy, &self.resolver_handle, domain)?;
        let ips = resolver_job
            .get_r1_or_first(self.config.happy_eyeballs.resolution_delay(), usize::MAX)
            .await?;
        strategy.pick_best(ips).ok_or(ResolveError::UnexpectedError(
            "no upstream ip can be selected",
        ))
    }

    async fn redirect_get_best(
        &self,
        redirect_result: Host,
        resolve_strategy: ResolveStrategy,
    ) -> Result<IpAddr, ResolveError> {
        match redirect_result {
            Host::Ip(ip) => Ok(ip),
            Host::Domain(new) => self.resolve_best(new, resolve_strategy).await,
        }
    }

    async fn select_upstream_addr(
        &self,
        ups: &UpstreamAddr,
        resolve_strategy: ResolveStrategy,
        task_notes: &ServerTaskNotes,
    ) -> Result<SocketAddr, ResolveError> {
        match ups.host() {
            Host::Ip(ip) => Ok(SocketAddr::new(*ip, ups.port())),
            Host::Domain(domain) => {
                if let Some(user_ctx) = task_notes.user_ctx() {
                    if let Some(redirect) = user_ctx.user().resolve_redirection() {
                        if let Some(v) = redirect.query_first(domain, resolve_strategy.query) {
                            return self
                                .redirect_get_best(v, resolve_strategy)
                                .await
                                .map(|ip| SocketAddr::new(ip, ups.port()));
                        }
                    }
                }

                if let Some(redirect) = &self.resolve_redirection {
                    if let Some(v) = redirect.query_first(domain, resolve_strategy.query) {
                        return self
                            .redirect_get_best(v, resolve_strategy)
                            .await
                            .map(|ip| SocketAddr::new(ip, ups.port()));
                    }
                }

                let ip = self.resolve_best(domain.clone(), resolve_strategy).await?;
                Ok(SocketAddr::new(ip, ups.port()))
            }
        }
    }

    async fn send_tcp_proxy_protocol_header<W>(
        &self,
        version: ProxyProtocolVersion,
        writer: &mut W,
        task_notes: &ServerTaskNotes,
        do_flush: bool,
    ) -> Result<(), TcpConnectError>
    where
        W: AsyncWrite + Unpin,
    {
        let mut encoder = ProxyProtocolEncoder::new(version);
        let bytes = encoder
            .encode_tcp(task_notes.client_addr(), task_notes.server_addr())
            .map_err(TcpConnectError::ProxyProtocolEncodeError)?;
        writer
            .write_all(bytes) // no need to flush data
            .await
            .map_err(TcpConnectError::ProxyProtocolWriteFailed)?;
        self.stats.tcp.io.add_out_bytes(bytes.len() as u64);
        if do_flush {
            writer
                .flush()
                .await
                .map_err(TcpConnectError::ProxyProtocolWriteFailed)?;
        }
        Ok(())
    }

    fn fetch_user_upstream_io_stats(
        &self,
        task_notes: &ServerTaskNotes,
    ) -> Vec<Arc<UserUpstreamTrafficStats>> {
        task_notes
            .user_ctx()
            .map(|ctx| ctx.fetch_upstream_traffic_stats(self.name(), self.stats.share_extra_tags()))
            .unwrap_or_default()
    }
}

#[async_trait]
impl Escaper for DirectFixedEscaper {
    fn name(&self) -> &NodeName {
        self.config.name()
    }

    fn get_escape_stats(&self) -> Option<ArcEscaperStats> {
        Some(self.stats.clone())
    }

    async fn publish(&self, _data: String) -> anyhow::Result<()> {
        Err(anyhow!("not implemented"))
    }

    async fn tcp_setup_connection(
        &self,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
        _audit_ctx: &mut AuditContext,
    ) -> TcpConnectResult {
        self.stats.interface.add_tcp_connect_attempted();
        tcp_notes.escaper.clone_from(&self.config.name);
        self.tcp_new_connection(task_conf, tcp_notes, task_notes, task_stats)
            .await
    }

    async fn tls_setup_connection(
        &self,
        task_conf: &TlsConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
        _audit_ctx: &mut AuditContext,
    ) -> TcpConnectResult {
        self.stats.interface.add_tls_connect_attempted();
        tcp_notes.escaper.clone_from(&self.config.name);
        self.tls_new_connection(task_conf, tcp_notes, task_notes, task_stats)
            .await
    }

    async fn udp_setup_connection(
        &self,
        task_conf: &UdpConnectTaskConf<'_>,
        udp_notes: &mut UdpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcUdpConnectTaskRemoteStats,
    ) -> UdpConnectResult {
        self.stats.interface.add_udp_connect_attempted();
        udp_notes.escaper.clone_from(&self.config.name);
        self.udp_connect_to(task_conf, udp_notes, task_notes, task_stats)
            .await
    }

    async fn udp_setup_relay(
        &self,
        task_conf: &UdpRelayTaskConf<'_>,
        udp_notes: &mut UdpRelayTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcUdpRelayTaskRemoteStats,
    ) -> UdpRelaySetupResult {
        self.stats.interface.add_udp_relay_session_attempted();
        udp_notes.escaper.clone_from(&self.config.name);
        self.udp_setup_relay(task_conf, task_notes, task_stats)
            .await
    }

    fn new_http_forward_context(&self, escaper: ArcEscaper) -> BoxHttpForwardContext {
        let ctx = DirectHttpForwardContext::new(self.stats.clone(), escaper);
        Box::new(ctx)
    }

    async fn new_ftp_connect_context(
        &self,
        escaper: ArcEscaper,
        task_conf: &TcpConnectTaskConf<'_>,
        _task_notes: &ServerTaskNotes,
    ) -> BoxFtpConnectContext {
        Box::new(DirectFtpConnectContext::new(
            escaper,
            task_conf.upstream.clone(),
        ))
    }
}

#[async_trait]
impl EscaperInternal for DirectFixedEscaper {
    fn _resolver(&self) -> &NodeName {
        self.config.resolver()
    }

    fn _depend_on_escaper(&self, _name: &NodeName) -> bool {
        false
    }

    fn _clone_config(&self) -> AnyEscaperConfig {
        let config = &*self.config;
        AnyEscaperConfig::DirectFixed(config.clone())
    }

    fn _reload(
        &self,
        config: AnyEscaperConfig,
        _registry: &mut EscaperRegistry,
    ) -> anyhow::Result<ArcEscaper> {
        let stats = Arc::clone(&self.stats);
        DirectFixedEscaper::prepare_reload(config, stats)
    }

    async fn _new_http_forward_connection(
        &self,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        self.stats.interface.add_http_forward_connection_attempted();
        tcp_notes.escaper.clone_from(&self.config.name);
        self.http_forward_new_connection(task_conf, tcp_notes, task_notes, task_stats)
            .await
    }

    async fn _new_https_forward_connection(
        &self,
        task_conf: &TlsConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        self.stats
            .interface
            .add_https_forward_connection_attempted();
        tcp_notes.escaper.clone_from(&self.config.name);
        self.https_forward_new_connection(task_conf, tcp_notes, task_notes, task_stats)
            .await
    }

    async fn _new_ftp_control_connection(
        &self,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcFtpTaskRemoteControlStats,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError> {
        self.stats.interface.add_ftp_over_http_request_attempted();
        self.stats.interface.add_ftp_control_connection_attempted();
        tcp_notes.escaper.clone_from(&self.config.name);
        self.new_ftp_control_connection(task_conf, tcp_notes, task_notes, task_stats)
            .await
    }

    async fn _new_ftp_transfer_connection(
        &self,
        task_conf: &TcpConnectTaskConf<'_>,
        transfer_tcp_notes: &mut TcpConnectTaskNotes,
        control_tcp_notes: &TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcFtpTaskRemoteTransferStats,
        ftp_server: &UpstreamAddr,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError> {
        self.stats.interface.add_ftp_transfer_connection_attempted();
        transfer_tcp_notes.escaper.clone_from(&self.config.name);

        self.new_ftp_transfer_connection(
            task_conf,
            transfer_tcp_notes,
            control_tcp_notes,
            task_notes,
            task_stats,
            ftp_server,
        )
        .await
    }
}
