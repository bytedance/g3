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

use std::collections::BTreeSet;
use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use rand::seq::SliceRandom;
use slog::Logger;

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_resolver::ResolveError;
use g3_socket::util::AddressFamily;
use g3_types::acl::AclNetworkRule;
use g3_types::metrics::MetricsName;
use g3_types::net::{Host, OpensslTlsClientConfig, UpstreamAddr};
use g3_types::resolve::{ResolveRedirection, ResolveStrategy};
use g3_types::route::EgressPathSelection;

use super::{
    ArcEscaper, ArcEscaperInternalStats, ArcEscaperStats, Escaper, EscaperInternal, EscaperStats,
};
use crate::auth::UserUpstreamTrafficStats;
use crate::config::escaper::direct_fixed::DirectFixedEscaperConfig;
use crate::config::escaper::{AnyEscaperConfig, EscaperConfig};
use crate::module::ftp_over_http::{
    AnyFtpConnectContextParam, ArcFtpTaskRemoteControlStats, ArcFtpTaskRemoteTransferStats,
    BoxFtpConnectContext, BoxFtpRemoteConnection, DirectFtpConnectContext,
    DirectFtpConnectContextParam,
};
use crate::module::http_forward::{
    ArcHttpForwardTaskRemoteStats, BoxHttpForwardConnection, BoxHttpForwardContext,
    DirectHttpForwardContext,
};
use crate::module::tcp_connect::{TcpConnectError, TcpConnectResult, TcpConnectTaskNotes};
use crate::module::udp_connect::{
    ArcUdpConnectTaskRemoteStats, UdpConnectResult, UdpConnectTaskNotes,
};
use crate::module::udp_relay::{
    ArcUdpRelayTaskRemoteStats, UdpRelaySetupResult, UdpRelayTaskNotes,
};
use crate::resolve::{ArcIntegratedResolverHandle, HappyEyeballsResolveJob};
use crate::serve::ServerTaskNotes;

mod stats;
use stats::DirectFixedEscaperStats;

mod ftp_connect;
mod http_forward;
mod tcp_connect;
mod tls_connect;
mod udp_connect;
mod udp_relay;

pub(super) struct DirectFixedEscaper {
    config: Arc<DirectFixedEscaperConfig>,
    stats: Arc<DirectFixedEscaperStats>,
    resolver_handle: ArcIntegratedResolverHandle,
    egress_net_filter: Arc<AclNetworkRule>,
    resolve_redirection: Option<ResolveRedirection>,
    escape_logger: Logger,
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

    pub(super) fn prepare_initial(config: AnyEscaperConfig) -> anyhow::Result<ArcEscaper> {
        if let AnyEscaperConfig::DirectFixed(config) = config {
            let stats = Arc::new(DirectFixedEscaperStats::new(config.name()));
            DirectFixedEscaper::new_obj(*config, stats)
        } else {
            Err(anyhow!("invalid escaper config type"))
        }
    }

    fn prepare_reload(
        config: AnyEscaperConfig,
        stats: Arc<DirectFixedEscaperStats>,
    ) -> anyhow::Result<ArcEscaper> {
        if let AnyEscaperConfig::DirectFixed(config) = config {
            DirectFixedEscaper::new_obj(*config, stats)
        } else {
            Err(anyhow!("invalid escaper config type"))
        }
    }

    fn get_bind_random(
        &self,
        family: AddressFamily,
        path_selection: &EgressPathSelection,
    ) -> Option<IpAddr> {
        let vec = match family {
            AddressFamily::Ipv4 => &self.config.bind4,
            AddressFamily::Ipv6 => &self.config.bind6,
        };
        match vec.len() {
            0 => None,
            1 => Some(vec[0]),
            n => {
                if self.config.enable_path_selection {
                    if let Some(i) = path_selection.select_by_index(n) {
                        return Some(vec[i]);
                    }
                }

                vec.choose(&mut rand::thread_rng()).copied()
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
        domain: &str,
        strategy: ResolveStrategy,
        task_notes: &ServerTaskNotes,
    ) -> Result<HappyEyeballsResolveJob, ResolveError> {
        if let Some(user_ctx) = task_notes.user_ctx() {
            if let Some(redirect) = user_ctx.user().resolve_redirection() {
                if let Some(v) = redirect.query_value(domain) {
                    return HappyEyeballsResolveJob::new_redirected(
                        strategy,
                        &self.resolver_handle,
                        v,
                    );
                }
            }
        }

        if let Some(redirect) = &self.resolve_redirection {
            if let Some(v) = redirect.query_value(domain) {
                return HappyEyeballsResolveJob::new_redirected(strategy, &self.resolver_handle, v);
            }
        }

        HappyEyeballsResolveJob::new_dyn(strategy, &self.resolver_handle, domain)
    }

    async fn resolve_best(
        &self,
        domain: &str,
        strategy: ResolveStrategy,
    ) -> Result<IpAddr, ResolveError> {
        let mut resolver_job =
            HappyEyeballsResolveJob::new_dyn(strategy, &self.resolver_handle, domain)?;
        let ips = resolver_job
            .get_r1_or_first(self.config.happy_eyeballs.resolution_delay(), usize::MAX)
            .await?;
        strategy.pick_best(ips).ok_or_else(|| {
            ResolveError::UnexpectedError("no upstream ip can be selected".to_string())
        })
    }

    async fn redirect_get_best(
        &self,
        redirect_result: Host,
        resolve_strategy: ResolveStrategy,
    ) -> Result<IpAddr, ResolveError> {
        match redirect_result {
            Host::Ip(ip) => Ok(ip),
            Host::Domain(new) => self.resolve_best(&new, resolve_strategy).await,
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

                let ip = self.resolve_best(domain, resolve_strategy).await?;
                Ok(SocketAddr::new(ip, ups.port()))
            }
        }
    }

    fn fetch_user_upstream_io_stats(
        &self,
        task_notes: &ServerTaskNotes,
    ) -> Vec<Arc<UserUpstreamTrafficStats>> {
        task_notes
            .user_ctx()
            .map(|ctx| ctx.fetch_upstream_traffic_stats(self.name(), self.stats.extra_tags()))
            .unwrap_or_default()
    }
}

#[async_trait]
impl Escaper for DirectFixedEscaper {
    fn name(&self) -> &MetricsName {
        self.config.name()
    }

    fn escaper_type(&self) -> &str {
        self.config.escaper_type()
    }

    fn get_escape_stats(&self) -> Option<ArcEscaperStats> {
        Some(Arc::clone(&self.stats) as ArcEscaperStats)
    }

    async fn publish(&self, _data: String) -> anyhow::Result<()> {
        Err(anyhow!("not implemented"))
    }

    async fn tcp_setup_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
    ) -> TcpConnectResult {
        self.stats.interface.add_tcp_connect_attempted();
        tcp_notes.escaper.clone_from(&self.config.name);
        self.tcp_new_connection(tcp_notes, task_notes, task_stats)
            .await
    }

    async fn tls_setup_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
        tls_config: &'a OpensslTlsClientConfig,
        tls_name: &'a str,
    ) -> TcpConnectResult {
        self.stats.interface.add_tls_connect_attempted();
        tcp_notes.escaper.clone_from(&self.config.name);
        self.tls_new_connection(tcp_notes, task_notes, task_stats, tls_config, tls_name)
            .await
    }

    async fn udp_setup_connection<'a>(
        &'a self,
        udp_notes: &'a mut UdpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcUdpConnectTaskRemoteStats,
    ) -> UdpConnectResult {
        self.stats.interface.add_udp_connect_attempted();
        udp_notes.escaper.clone_from(&self.config.name);
        self.udp_connect_to(udp_notes, task_notes, task_stats).await
    }

    async fn udp_setup_relay<'a>(
        &'a self,
        udp_notes: &'a mut UdpRelayTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcUdpRelayTaskRemoteStats,
    ) -> UdpRelaySetupResult {
        self.stats.interface.add_udp_relay_session_attempted();
        udp_notes.escaper.clone_from(&self.config.name);
        self.udp_setup_relay(udp_notes, task_notes, task_stats)
            .await
    }

    fn new_http_forward_context(&self, escaper: ArcEscaper) -> BoxHttpForwardContext {
        let ctx = DirectHttpForwardContext::new(
            Arc::clone(&self.stats) as ArcEscaperInternalStats,
            escaper,
        );
        Box::new(ctx)
    }

    async fn new_ftp_connect_context<'a>(
        &'a self,
        escaper: ArcEscaper,
        _task_notes: &'a ServerTaskNotes,
        upstream: &'a UpstreamAddr,
    ) -> BoxFtpConnectContext {
        Box::new(DirectFtpConnectContext::new(escaper, upstream.clone()))
    }
}

#[async_trait]
impl EscaperInternal for DirectFixedEscaper {
    fn _resolver(&self) -> &MetricsName {
        self.config.resolver()
    }

    fn _dependent_escaper(&self) -> Option<BTreeSet<MetricsName>> {
        None
    }

    fn _clone_config(&self) -> AnyEscaperConfig {
        let config = &*self.config;
        AnyEscaperConfig::DirectFixed(Box::new(config.clone()))
    }

    fn _update_config_in_place(
        &self,
        _flags: u64,
        _config: AnyEscaperConfig,
    ) -> anyhow::Result<()> {
        Ok(())
    }

    async fn _lock_safe_reload(&self, config: AnyEscaperConfig) -> anyhow::Result<ArcEscaper> {
        let stats = Arc::clone(&self.stats);
        DirectFixedEscaper::prepare_reload(config, stats)
    }

    async fn _check_out_next_escaper(
        &self,
        _task_notes: &ServerTaskNotes,
        _upstream: &UpstreamAddr,
    ) -> Option<ArcEscaper> {
        None
    }

    async fn _new_http_forward_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        self.stats.interface.add_http_forward_connection_attempted();
        tcp_notes.escaper.clone_from(&self.config.name);
        self.http_forward_new_connection(tcp_notes, task_notes, task_stats)
            .await
    }

    async fn _new_https_forward_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
        tls_config: &'a OpensslTlsClientConfig,
        tls_name: &'a str,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        self.stats
            .interface
            .add_https_forward_connection_attempted();
        tcp_notes.escaper.clone_from(&self.config.name);
        self.https_forward_new_connection(tcp_notes, task_notes, task_stats, tls_config, tls_name)
            .await
    }

    async fn _new_ftp_control_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcFtpTaskRemoteControlStats,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError> {
        self.stats.interface.add_ftp_over_http_request_attempted();
        self.stats.interface.add_ftp_control_connection_attempted();
        tcp_notes.escaper.clone_from(&self.config.name);
        self.new_ftp_control_connection(tcp_notes, task_notes, task_stats)
            .await
    }

    async fn _new_ftp_transfer_connection<'a>(
        &'a self,
        transfer_tcp_notes: &'a mut TcpConnectTaskNotes,
        control_tcp_notes: &'a TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcFtpTaskRemoteTransferStats,
        mut context: AnyFtpConnectContextParam,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError> {
        self.stats.interface.add_ftp_transfer_connection_attempted();
        transfer_tcp_notes.escaper.clone_from(&self.config.name);
        match context.downcast_mut::<DirectFtpConnectContextParam>() {
            Some(_ctx) => {
                self.new_ftp_transfer_connection(
                    transfer_tcp_notes,
                    control_tcp_notes,
                    task_notes,
                    task_stats,
                )
                .await
            }
            None => Err(TcpConnectError::EscaperNotUsable),
        }
    }
}
