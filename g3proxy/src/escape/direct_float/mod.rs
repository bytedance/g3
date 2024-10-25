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
use arc_swap::ArcSwap;
use async_trait::async_trait;
use chrono::Utc;
use log::warn;
use slog::Logger;
use tokio::time::Instant;

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_resolver::ResolveError;
use g3_socket::util::AddressFamily;
use g3_types::acl::AclNetworkRule;
use g3_types::metrics::MetricsName;
use g3_types::net::{Host, UpstreamAddr};
use g3_types::resolve::{ResolveRedirection, ResolveStrategy};

use super::{
    ArcEscaper, ArcEscaperInternalStats, ArcEscaperStats, Escaper, EscaperInternal, EscaperStats,
};
use crate::audit::AuditContext;
use crate::auth::UserUpstreamTrafficStats;
use crate::config::escaper::direct_float::{BindSet, DirectFloatBindIp, DirectFloatEscaperConfig};
use crate::config::escaper::{AnyEscaperConfig, EscaperConfig};
use crate::escape::direct_fixed::DirectFixedEscaperStats;
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
    ArcUdpConnectTaskRemoteStats, UdpConnectResult, UdpConnectTaskNotes,
};
use crate::module::udp_relay::{
    ArcUdpRelayTaskRemoteStats, UdpRelaySetupResult, UdpRelayTaskNotes,
};
use crate::resolve::{ArcIntegratedResolverHandle, HappyEyeballsResolveJob};
use crate::serve::ServerTaskNotes;

mod publish;

mod ftp_connect;
mod http_forward;
mod tcp_connect;
mod tls_connect;
mod udp_connect;
mod udp_relay;

pub(super) struct DirectFloatEscaper {
    config: Arc<DirectFloatEscaperConfig>,
    stats: Arc<DirectFixedEscaperStats>,
    resolver_handle: ArcIntegratedResolverHandle,
    egress_net_filter: Arc<AclNetworkRule>,
    resolve_redirection: Option<ResolveRedirection>,
    bind_v4: ArcSwap<BindSet>,
    bind_v6: ArcSwap<BindSet>,
    escape_logger: Logger,
}

impl DirectFloatEscaper {
    async fn new_obj(
        config: DirectFloatEscaperConfig,
        stats: Arc<DirectFixedEscaperStats>,
        bind_v4: Option<Arc<BindSet>>,
        bind_v6: Option<Arc<BindSet>>,
    ) -> anyhow::Result<ArcEscaper> {
        let resolver_handle = crate::resolve::get_handle(config.resolver())?;
        let egress_net_filter = Arc::new(config.egress_net_filter.build());

        let resolve_redirection = config
            .resolve_redirection
            .as_ref()
            .map(|builder| builder.build());

        let escape_logger = config.get_escape_logger();

        let config = Arc::new(config);

        let bind_v4 = match bind_v4 {
            Some(binds) => binds,
            None => {
                let bind_set = publish::load_ipv4_from_cache(&config)
                    .await
                    .unwrap_or_else(|e| {
                        warn!(
                            "failed to load cached ipv4 addr for escaper {}: {:?}",
                            config.name, e
                        );
                        BindSet::new(AddressFamily::Ipv4)
                    });
                Arc::new(bind_set)
            }
        };
        let bind_v6 = match bind_v6 {
            Some(binds) => binds,
            None => {
                let bind_set = publish::load_ipv6_from_cache(&config)
                    .await
                    .unwrap_or_else(|e| {
                        warn!(
                            "failed to load cached ipv6 addr for escaper {}: {:?}",
                            config.name, e
                        );
                        BindSet::new(AddressFamily::Ipv6)
                    });
                Arc::new(bind_set)
            }
        };

        stats.set_extra_tags(config.extra_metrics_tags.clone());

        let escaper = DirectFloatEscaper {
            config,
            stats,
            resolver_handle,
            egress_net_filter,
            resolve_redirection,
            bind_v4: ArcSwap::new(bind_v4),
            bind_v6: ArcSwap::new(bind_v6),
            escape_logger,
        };

        Ok(Arc::new(escaper))
    }

    pub(super) async fn prepare_initial(
        config: DirectFloatEscaperConfig,
    ) -> anyhow::Result<ArcEscaper> {
        let stats = Arc::new(DirectFixedEscaperStats::new(config.name()));
        DirectFloatEscaper::new_obj(config, stats, None, None).await
    }

    async fn prepare_reload(
        config: AnyEscaperConfig,
        stats: Arc<DirectFixedEscaperStats>,
        bind_v4: Option<Arc<BindSet>>,
        bind_v6: Option<Arc<BindSet>>,
    ) -> anyhow::Result<ArcEscaper> {
        if let AnyEscaperConfig::DirectFloat(config) = config {
            DirectFloatEscaper::new_obj(*config, stats, bind_v4, bind_v6).await
        } else {
            Err(anyhow!("invalid escaper config type"))
        }
    }

    fn parse_dyn_bind_ip(&self, value: &serde_json::Value) -> anyhow::Result<DirectFloatBindIp> {
        let instant_now = Instant::now();
        let datetime_now = Utc::now();
        DirectFloatBindIp::parse_json(value, instant_now, datetime_now)?
            .ok_or_else(|| anyhow!("expired bind IP json value"))
    }

    fn select_bind_again_from_escaper(&self, ip: IpAddr) -> anyhow::Result<DirectFloatBindIp> {
        let bind_set = match ip {
            IpAddr::V4(_) => self.bind_v4.load(),
            IpAddr::V6(_) => self.bind_v6.load(),
        };
        bind_set
            .select_again(ip)
            .ok_or_else(|| anyhow!("no bind IP {ip} found at escaper level"))
    }

    fn select_bind_again(
        &self,
        ip: IpAddr,
        task_notes: &ServerTaskNotes,
    ) -> anyhow::Result<DirectFloatBindIp> {
        if let Some(path_selection) = task_notes.egress_path() {
            if let Some(id) = path_selection.select_matched_id(self.name().as_str()) {
                let bind_set = match ip {
                    IpAddr::V4(_) => self.bind_v4.load(),
                    IpAddr::V6(_) => self.bind_v6.load(),
                };
                return bind_set
                    .select_named_bind(id)
                    .ok_or_else(|| anyhow!("no bind IP with ID {id} found at escaper level"));
            }

            if let Some(value) = path_selection.select_matched_value(self.name().as_str()) {
                return self.parse_dyn_bind_ip(value);
            }
        }

        self.select_bind_again_from_escaper(ip)
    }

    fn select_bind_from_escaper(&self, family: AddressFamily) -> anyhow::Result<DirectFloatBindIp> {
        let bind_set = match family {
            AddressFamily::Ipv4 => self.bind_v4.load(),
            AddressFamily::Ipv6 => self.bind_v6.load(),
        };
        bind_set
            .select_random_bind()
            .ok_or_else(|| anyhow!("no {family} bind IP available at escaper level"))
    }

    fn select_bind(
        &self,
        family: AddressFamily,
        task_notes: &ServerTaskNotes,
    ) -> anyhow::Result<DirectFloatBindIp> {
        if let Some(path_selection) = task_notes.egress_path() {
            if let Some(id) = path_selection.select_matched_id(self.name().as_str()) {
                let bind_set = match family {
                    AddressFamily::Ipv4 => self.bind_v4.load(),
                    AddressFamily::Ipv6 => self.bind_v6.load(),
                };
                return bind_set
                    .select_named_bind(id)
                    .ok_or_else(|| anyhow!("no bind IP with ID {id} found at escaper level"));
            }

            if let Some(value) = path_selection.select_matched_value(self.name().as_str()) {
                return self.parse_dyn_bind_ip(value);
            }
        }

        self.select_bind_from_escaper(family)
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
impl Escaper for DirectFloatEscaper {
    fn name(&self) -> &MetricsName {
        self.config.name()
    }

    fn escaper_type(&self) -> &str {
        self.config.escaper_type()
    }

    fn get_escape_stats(&self) -> Option<ArcEscaperStats> {
        Some(Arc::clone(&self.stats) as ArcEscaperStats)
    }

    async fn publish(&self, data: String) -> anyhow::Result<()> {
        publish::publish_records(&self.config, &self.bind_v4, &self.bind_v6, data).await
    }

    async fn tcp_setup_connection<'a>(
        &'a self,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
        _audit_ctx: &'a mut AuditContext,
    ) -> TcpConnectResult {
        self.stats.interface.add_tcp_connect_attempted();
        tcp_notes.escaper.clone_from(&self.config.name);
        self.tcp_new_connection(task_conf, tcp_notes, task_notes, task_stats)
            .await
    }

    async fn tls_setup_connection<'a>(
        &'a self,
        task_conf: &TlsConnectTaskConf<'_>,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
        _audit_ctx: &'a mut AuditContext,
    ) -> TcpConnectResult {
        self.stats.interface.add_tls_connect_attempted();
        tcp_notes.escaper.clone_from(&self.config.name);
        self.tls_new_connection(task_conf, tcp_notes, task_notes, task_stats)
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
        task_conf: &TcpConnectTaskConf<'_>,
        _task_notes: &'a ServerTaskNotes,
    ) -> BoxFtpConnectContext {
        Box::new(DirectFtpConnectContext::new(
            escaper,
            task_conf.upstream.clone(),
        ))
    }
}

#[async_trait]
impl EscaperInternal for DirectFloatEscaper {
    fn _resolver(&self) -> &MetricsName {
        self.config.resolver()
    }

    fn _dependent_escaper(&self) -> Option<BTreeSet<MetricsName>> {
        None
    }

    fn _clone_config(&self) -> AnyEscaperConfig {
        let config = &*self.config;
        AnyEscaperConfig::DirectFloat(Box::new(config.clone()))
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
        let bind_v4 = self.bind_v4.load_full();
        let bind_v6 = self.bind_v6.load_full();

        DirectFloatEscaper::prepare_reload(config, stats, Some(bind_v4), Some(bind_v6)).await
    }

    async fn _new_http_forward_connection<'a>(
        &'a self,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        self.stats.interface.add_http_forward_connection_attempted();
        tcp_notes.escaper.clone_from(&self.config.name);
        self.http_forward_new_connection(task_conf, tcp_notes, task_notes, task_stats)
            .await
    }

    async fn _new_https_forward_connection<'a>(
        &'a self,
        task_conf: &TlsConnectTaskConf<'_>,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        self.stats
            .interface
            .add_https_forward_connection_attempted();
        tcp_notes.escaper.clone_from(&self.config.name);
        self.https_forward_new_connection(task_conf, tcp_notes, task_notes, task_stats)
            .await
    }

    async fn _new_ftp_control_connection<'a>(
        &'a self,
        task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcFtpTaskRemoteControlStats,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError> {
        self.stats.interface.add_ftp_over_http_request_attempted();
        self.stats.interface.add_ftp_control_connection_attempted();
        tcp_notes.escaper.clone_from(&self.config.name);
        self.new_ftp_control_connection(task_conf, tcp_notes, task_notes, task_stats)
            .await
    }

    async fn _new_ftp_transfer_connection<'a>(
        &'a self,
        task_conf: &TcpConnectTaskConf<'_>,
        transfer_tcp_notes: &'a mut TcpConnectTaskNotes,
        control_tcp_notes: &'a TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
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

    fn _trick_float_weight(&self) -> u8 {
        let bind_v4 = self.bind_v4.load();
        if let Some(bind) = bind_v4.select_stable_bind() {
            let alive_minutes = bind.expected_alive_minutes();
            return u8::try_from(alive_minutes).unwrap_or(u8::MAX);
        }

        let bind_v6 = self.bind_v6.load();
        if let Some(bind) = bind_v6.select_stable_bind() {
            let alive_minutes = bind.expected_alive_minutes();
            return u8::try_from(alive_minutes).unwrap_or(u8::MAX);
        }

        0
    }
}
