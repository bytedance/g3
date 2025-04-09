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
use std::sync::Arc;

use anyhow::{Context, anyhow};
use arc_swap::ArcSwap;
use async_trait::async_trait;
use log::warn;
use slog::Logger;
use tokio::sync::mpsc;

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_types::metrics::NodeName;
use g3_types::net::{OpensslClientConfig, UpstreamAddr};

use super::{ArcEscaper, ArcEscaperStats, Escaper, EscaperInternal, EscaperRegistry, EscaperStats};
use crate::audit::AuditContext;
use crate::auth::UserUpstreamTrafficStats;
use crate::config::escaper::proxy_float::ProxyFloatEscaperConfig;
use crate::config::escaper::{AnyEscaperConfig, EscaperConfig};
use crate::module::ftp_over_http::{
    ArcFtpTaskRemoteControlStats, ArcFtpTaskRemoteTransferStats, BoxFtpConnectContext,
    BoxFtpRemoteConnection, DenyFtpConnectContext,
};
use crate::module::http_forward::{
    ArcHttpForwardTaskRemoteStats, BoxHttpForwardConnection, BoxHttpForwardContext,
    DirectHttpForwardContext,
};
use crate::module::tcp_connect::{
    TcpConnectError, TcpConnectResult, TcpConnectTaskConf, TcpConnectTaskNotes, TlsConnectTaskConf,
};
use crate::module::udp_connect::{
    ArcUdpConnectTaskRemoteStats, UdpConnectError, UdpConnectResult, UdpConnectTaskConf,
    UdpConnectTaskNotes,
};
use crate::module::udp_relay::{
    ArcUdpRelayTaskRemoteStats, UdpRelaySetupError, UdpRelaySetupResult, UdpRelayTaskConf,
    UdpRelayTaskNotes,
};
use crate::serve::ServerTaskNotes;

mod stats;
use stats::ProxyFloatEscaperStats;

mod peer;
use peer::{ArcNextProxyPeer, NextProxyPeer, PeerSet};

mod source;

mod tcp_connect;
mod tls_connect;
mod tls_handshake;

pub(super) struct ProxyFloatEscaper {
    config: Arc<ProxyFloatEscaperConfig>,
    stats: Arc<ProxyFloatEscaperStats>,
    quit_job_sender: Option<mpsc::Sender<()>>,
    peers: Arc<ArcSwap<PeerSet>>,
    tls_config: Arc<OpensslClientConfig>,
    escape_logger: Logger,
}

impl ProxyFloatEscaper {
    fn new_obj(
        config: ProxyFloatEscaperConfig,
        stats: Arc<ProxyFloatEscaperStats>,
        peers: ArcSwap<PeerSet>,
    ) -> anyhow::Result<ArcEscaper> {
        let escape_logger = config.get_escape_logger();

        let tls_config = config
            .tls_config
            .build()
            .context("failed to setup tls client config")?;

        let config = Arc::new(config);
        let peers = Arc::new(peers);
        let quit_job_sender = source::new_job(Arc::clone(&config), Arc::clone(&peers))?;

        stats.set_extra_tags(config.extra_metrics_tags.clone());

        let escaper = ProxyFloatEscaper {
            config,
            stats,
            quit_job_sender,
            peers,
            tls_config: Arc::new(tls_config),
            escape_logger,
        };

        Ok(Arc::new(escaper))
    }

    pub(super) async fn prepare_initial(
        config: ProxyFloatEscaperConfig,
    ) -> anyhow::Result<ArcEscaper> {
        let peers = source::load_cached_peers(&config)
            .await
            .unwrap_or_else(|e| {
                warn!(
                    "failed to load cached peers for escaper {}: {e:?}",
                    config.name
                );
                PeerSet::default()
            });
        let stats = Arc::new(ProxyFloatEscaperStats::new(config.name()));
        ProxyFloatEscaper::new_obj(config, stats, ArcSwap::from_pointee(peers))
    }

    fn prepare_reload(
        config: AnyEscaperConfig,
        stats: Arc<ProxyFloatEscaperStats>,
        peers: Arc<PeerSet>,
    ) -> anyhow::Result<ArcEscaper> {
        if let AnyEscaperConfig::ProxyFloat(config) = config {
            ProxyFloatEscaper::new_obj(config, stats, ArcSwap::new(peers))
        } else {
            Err(anyhow!("invalid escaper config type"))
        }
    }

    fn fetch_user_upstream_io_stats(
        &self,
        task_notes: &ServerTaskNotes,
    ) -> Vec<Arc<UserUpstreamTrafficStats>> {
        task_notes
            .user_ctx()
            .map(|ctx| {
                ctx.fetch_upstream_traffic_stats(self.stats.name(), self.stats.share_extra_tags())
            })
            .unwrap_or_default()
    }

    fn parse_dyn_peer(&self, value: &serde_json::Value) -> anyhow::Result<ArcNextProxyPeer> {
        peer::parse_peer(&self.config, value)?.ok_or_else(|| anyhow!("expired peer json value"))
    }

    fn select_peer_from_escaper(&self) -> Option<ArcNextProxyPeer> {
        let peer_set = self.peers.load();
        peer_set.select_random_peer()
    }

    fn select_peer(&self, task_notes: &ServerTaskNotes) -> anyhow::Result<ArcNextProxyPeer> {
        if let Some(path_selection) = task_notes.egress_path() {
            if let Some(id) = path_selection.select_matched_id(self.name().as_str()) {
                let peer_set = self.peers.load();
                let peer = peer_set
                    .select_named_peer(id)
                    .ok_or_else(|| anyhow!("no peer with id {id} found in local cache"))?;
                return if peer.is_expired() {
                    Err(anyhow!("peer {id} is expired"))
                } else {
                    Ok(peer)
                };
            }

            if let Some(value) = path_selection.select_matched_value(self.name().as_str()) {
                return self.parse_dyn_peer(value);
            }
        }

        self.select_peer_from_escaper()
            .ok_or_else(|| anyhow!("no peer can be selected from escaper config"))
    }
}

#[async_trait]
impl Escaper for ProxyFloatEscaper {
    fn name(&self) -> &NodeName {
        self.config.name()
    }

    fn escaper_type(&self) -> &str {
        self.config.escaper_type()
    }

    fn get_escape_stats(&self) -> Option<ArcEscaperStats> {
        Some(self.stats.clone())
    }

    async fn publish(&self, data: String) -> anyhow::Result<()> {
        source::publish_peers(&self.config, &self.peers, data).await
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
        let peer = self
            .select_peer(task_notes)
            .map_err(TcpConnectError::EscaperNotUsable)?;
        peer.tcp_setup_connection(self, task_conf, tcp_notes, task_notes, task_stats)
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
        let peer = self
            .select_peer(task_notes)
            .map_err(TcpConnectError::EscaperNotUsable)?;
        peer.tls_setup_connection(self, task_conf, tcp_notes, task_notes, task_stats)
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
        let peer = self
            .select_peer(task_notes)
            .map_err(UdpConnectError::EscaperNotUsable)?;
        peer.udp_setup_connection(self, task_conf, udp_notes, task_notes, task_stats)
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
        let peer = self
            .select_peer(task_notes)
            .map_err(UdpRelaySetupError::EscaperNotUsable)?;
        peer.udp_setup_relay(self, task_conf, udp_notes, task_notes, task_stats)
            .await
    }

    fn new_http_forward_context(&self, escaper: ArcEscaper) -> BoxHttpForwardContext {
        let ctx = DirectHttpForwardContext::new(self.stats.clone(), escaper);
        Box::new(ctx)
    }

    async fn new_ftp_connect_context(
        &self,
        _escaper: ArcEscaper,
        _task_conf: &TcpConnectTaskConf<'_>,
        _task_notes: &ServerTaskNotes,
    ) -> BoxFtpConnectContext {
        Box::new(DenyFtpConnectContext::new(self.name(), None))
    }
}

#[async_trait]
impl EscaperInternal for ProxyFloatEscaper {
    fn _resolver(&self) -> &NodeName {
        Default::default()
    }

    fn _dependent_escaper(&self) -> Option<BTreeSet<NodeName>> {
        None
    }

    fn _clone_config(&self) -> AnyEscaperConfig {
        let config = &*self.config;
        AnyEscaperConfig::ProxyFloat(config.clone())
    }

    fn _reload(
        &self,
        config: AnyEscaperConfig,
        _registry: &mut EscaperRegistry,
    ) -> anyhow::Result<ArcEscaper> {
        let stats = Arc::clone(&self.stats);
        // copy the old peers, they may be a little outdated at this stage
        // as we haven't stopped the old job
        let peers = self.peers.load_full();
        ProxyFloatEscaper::prepare_reload(config, stats, peers)
    }

    fn _clean_to_offline(&self) {
        if let Some(sender) = &self.quit_job_sender {
            let _ = sender.try_send(());
        }
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
        let peer = self
            .select_peer(task_notes)
            .map_err(TcpConnectError::EscaperNotUsable)?;
        peer.new_http_forward_connection(self, task_conf, tcp_notes, task_notes, task_stats)
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
        let peer = self
            .select_peer(task_notes)
            .map_err(TcpConnectError::EscaperNotUsable)?;
        peer.new_https_forward_connection(self, task_conf, tcp_notes, task_notes, task_stats)
            .await
    }

    async fn _new_ftp_control_connection(
        &self,
        _task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        _task_notes: &ServerTaskNotes,
        _task_stats: ArcFtpTaskRemoteControlStats,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError> {
        self.stats.interface.add_ftp_over_http_request_attempted();
        self.stats.interface.add_ftp_control_connection_attempted();
        tcp_notes.escaper.clone_from(&self.config.name);
        Err(TcpConnectError::MethodUnavailable)
    }

    async fn _new_ftp_transfer_connection(
        &self,
        _task_conf: &TcpConnectTaskConf<'_>,
        transfer_tcp_notes: &mut TcpConnectTaskNotes,
        _control_tcp_notes: &TcpConnectTaskNotes,
        _task_notes: &ServerTaskNotes,
        _task_stats: ArcFtpTaskRemoteTransferStats,
        _ftp_server: &UpstreamAddr,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError> {
        self.stats.interface.add_ftp_transfer_connection_attempted();
        transfer_tcp_notes.escaper.clone_from(&self.config.name);
        Err(TcpConnectError::MethodUnavailable)
    }

    fn _trick_float_weight(&self) -> u8 {
        let peer_set = self.peers.load();
        peer_set
            .select_stable_peer()
            .map(|peer| {
                let alive_minutes = peer.expected_alive_minutes();
                u8::try_from(alive_minutes).unwrap_or(u8::MAX)
            })
            .unwrap_or(0)
    }
}
