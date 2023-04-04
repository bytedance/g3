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
use std::convert::TryFrom;
use std::net::IpAddr;
use std::sync::Arc;

use anyhow::anyhow;
use arc_swap::ArcSwap;
use async_trait::async_trait;
use log::warn;
use rand::seq::SliceRandom;
use slog::Logger;

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_resolver::ResolveError;
use g3_socket::util::AddressFamily;
use g3_types::acl::AclNetworkRule;
use g3_types::metrics::MetricsName;
use g3_types::net::{OpensslTlsClientConfig, UpstreamAddr};
use g3_types::resolve::{ResolveRedirection, ResolveStrategy};

use super::{
    ArcEscaper, ArcEscaperInternalStats, ArcEscaperStats, Escaper, EscaperInternal, EscaperStats,
};
use crate::auth::UserUpstreamTrafficStats;
use crate::config::escaper::direct_float::DirectFloatEscaperConfig;
use crate::config::escaper::{AnyEscaperConfig, EscaperConfig};
use crate::module::ftp_over_http::{
    AnyFtpConnectContextParam, ArcFtpTaskRemoteControlStats, ArcFtpTaskRemoteTransferStats,
    BoxFtpConnectContext, BoxFtpRemoteHttpConnection, DirectFtpConnectContext,
    DirectFtpConnectContextParam,
};
use crate::module::http_forward::{
    ArcHttpForwardTaskRemoteStats, BoxHttpForwardConnection, BoxHttpForwardContext,
    DirectHttpForwardContext,
};
use crate::module::tcp_connect::{TcpConnectError, TcpConnectResult, TcpConnectTaskNotes};
use crate::module::udp_connect::{
    ArcUdpConnectTaskRemoteStats, UdpConnectError, UdpConnectResult, UdpConnectTaskNotes,
};
use crate::module::udp_relay::{
    ArcUdpRelayTaskRemoteStats, UdpRelaySetupError, UdpRelaySetupResult, UdpRelayTaskNotes,
};
use crate::resolve::{ArcIntegratedResolverHandle, HappyEyeballsResolveJob};
use crate::serve::ServerTaskNotes;

mod bind;
use bind::DirectFloatBindIp;

mod publish;

mod stats;
use stats::DirectFloatEscaperStats;

mod ftp_connect;
mod http_forward;
mod tcp_connect;
mod tls_connect;

pub(super) struct DirectFloatEscaper {
    config: Arc<DirectFloatEscaperConfig>,
    stats: Arc<DirectFloatEscaperStats>,
    resolver_handle: ArcIntegratedResolverHandle,
    egress_net_filter: AclNetworkRule,
    resolve_redirection: Option<ResolveRedirection>,
    bind_v4: ArcSwap<Box<[DirectFloatBindIp]>>,
    bind_v6: ArcSwap<Box<[DirectFloatBindIp]>>,
    escape_logger: Logger,
}

impl DirectFloatEscaper {
    async fn new_obj(
        config: DirectFloatEscaperConfig,
        stats: Arc<DirectFloatEscaperStats>,
        bind_v4: Option<Arc<Box<[DirectFloatBindIp]>>>,
        bind_v6: Option<Arc<Box<[DirectFloatBindIp]>>>,
    ) -> anyhow::Result<ArcEscaper> {
        let resolver_handle = crate::resolve::get_handle(config.resolver())?;
        let egress_net_filter = config.egress_net_filter.build();

        let resolve_redirection = config
            .resolve_redirection
            .as_ref()
            .map(|builder| builder.build());

        let escape_logger = config.get_escape_logger();

        let config = Arc::new(config);

        let bind_v4 = match bind_v4 {
            Some(binds) => binds,
            None => {
                let vec = publish::load_ipv4_from_cache(&config)
                    .await
                    .unwrap_or_else(|e| {
                        warn!(
                            "failed to load cached ipv4 addr for escaper {}: {:?}",
                            config.name, e
                        );
                        Vec::new()
                    });
                Arc::new(vec.into_boxed_slice())
            }
        };
        let bind_v6 = match bind_v6 {
            Some(binds) => binds,
            None => {
                let vec = publish::load_ipv6_from_cache(&config)
                    .await
                    .unwrap_or_else(|e| {
                        warn!(
                            "failed to load cached ipv6 addr for escaper {}: {:?}",
                            config.name, e
                        );
                        Vec::new()
                    });
                Arc::new(vec.into_boxed_slice())
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

    pub(super) async fn prepare_initial(config: AnyEscaperConfig) -> anyhow::Result<ArcEscaper> {
        if let AnyEscaperConfig::DirectFloat(config) = config {
            let stats = Arc::new(DirectFloatEscaperStats::new(config.name()));
            DirectFloatEscaper::new_obj(*config, stats, None, None).await
        } else {
            Err(anyhow!("invalid escaper config type"))
        }
    }

    async fn prepare_reload(
        config: AnyEscaperConfig,
        stats: Arc<DirectFloatEscaperStats>,
        bind_v4: Option<Arc<Box<[DirectFloatBindIp]>>>,
        bind_v6: Option<Arc<Box<[DirectFloatBindIp]>>>,
    ) -> anyhow::Result<ArcEscaper> {
        if let AnyEscaperConfig::DirectFloat(config) = config {
            DirectFloatEscaper::new_obj(*config, stats, bind_v4, bind_v6).await
        } else {
            Err(anyhow!("invalid escaper config type"))
        }
    }

    fn get_bind_again(&self, ip: IpAddr) -> Option<DirectFloatBindIp> {
        let vec = match ip {
            IpAddr::V4(_) => self.bind_v4.load(),
            IpAddr::V6(_) => self.bind_v6.load(),
        };
        let vec = vec.as_ref();
        for v in vec.as_ref() {
            if v.ip == ip {
                return Some(v.clone());
            }
        }
        None
    }

    fn get_bind_random(&self, family: AddressFamily) -> Option<DirectFloatBindIp> {
        let vec = match family {
            AddressFamily::Ipv4 => self.bind_v4.load(),
            AddressFamily::Ipv6 => self.bind_v6.load(),
        };
        let vec = vec.as_ref();
        match vec.len() {
            0 => None,
            1 => {
                let bind = &vec[0];
                if bind.is_expired() {
                    None
                } else {
                    Some(bind.clone())
                }
            }
            _ => {
                let mut rng = rand::thread_rng();
                if let Ok(bind) =
                    vec.choose_weighted(&mut rng, |bind| i32::from(!bind.is_expired()))
                {
                    Some(bind.clone())
                } else {
                    None
                }
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
        _task_notes: &'a ServerTaskNotes,
        _task_stats: ArcUdpConnectTaskRemoteStats,
    ) -> UdpConnectResult {
        self.stats.interface.add_udp_connect_attempted();
        udp_notes.escaper.clone_from(&self.config.name);
        Err(UdpConnectError::MethodUnavailable)
    }

    async fn udp_setup_relay<'a>(
        &'a self,
        udp_notes: &'a mut UdpRelayTaskNotes,
        _task_notes: &'a ServerTaskNotes,
        _task_stats: ArcUdpRelayTaskRemoteStats,
    ) -> UdpRelaySetupResult {
        self.stats.interface.add_udp_relay_session_attempted();
        udp_notes.escaper.clone_from(&self.config.name);
        Err(UdpRelaySetupError::MethodUnavailable)
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
    ) -> Result<BoxFtpRemoteHttpConnection, TcpConnectError> {
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
    ) -> Result<BoxFtpRemoteHttpConnection, TcpConnectError> {
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

    fn _trick_float_weight(&self) -> u8 {
        let mut vec = self.bind_v4.load();
        if vec.len() == 0 {
            // the v4 and v6 binding should be in sync in most cases if both available.
            // If v4 available, then v6 may be unavailable or just be the same.
            vec = self.bind_v6.load();
        }
        let vec = vec.as_ref();
        if vec.len() == 1 {
            let bind = &vec[0];
            let alive_minutes = bind.expected_alive_minutes();
            u8::try_from(alive_minutes).unwrap_or(u8::MAX)
        } else {
            0
        }
    }
}
