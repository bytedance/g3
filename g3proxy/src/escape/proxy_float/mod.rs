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
use std::sync::Arc;

use anyhow::{anyhow, Context};
use arc_swap::ArcSwap;
use async_trait::async_trait;
use futures_util::future::AbortHandle;
use log::warn;
use rand::seq::IteratorRandom;
use serde_json::Value;
use slog::Logger;

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_types::metrics::MetricsName;
use g3_types::net::{OpensslClientConfig, UpstreamAddr};

use super::{ArcEscaper, ArcEscaperStats, Escaper, EscaperInternal};
use crate::config::escaper::proxy_float::ProxyFloatEscaperConfig;
use crate::config::escaper::{AnyEscaperConfig, EscaperConfig};
use crate::module::ftp_over_http::{
    AnyFtpConnectContextParam, ArcFtpTaskRemoteControlStats, ArcFtpTaskRemoteTransferStats,
    BoxFtpConnectContext, BoxFtpRemoteConnection, DenyFtpConnectContext,
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
use crate::serve::ServerTaskNotes;

mod stats;
use stats::ProxyFloatEscaperStats;

mod peer;
use peer::{ArcNextProxyPeer, PeerSet};
mod source;

pub(super) struct ProxyFloatEscaper {
    config: Arc<ProxyFloatEscaperConfig>,
    stats: Arc<ProxyFloatEscaperStats>,
    source_job_handler: Option<AbortHandle>,
    peers: Arc<ArcSwap<PeerSet>>,
    tls_config: Option<Arc<OpensslClientConfig>>,
    escape_logger: Logger,
}

impl Drop for ProxyFloatEscaper {
    fn drop(&mut self) {
        if let Some(handler) = self.source_job_handler.take() {
            handler.abort();
        }
    }
}

impl ProxyFloatEscaper {
    async fn new_obj(
        config: ProxyFloatEscaperConfig,
        stats: Arc<ProxyFloatEscaperStats>,
        peers: Option<Arc<PeerSet>>,
    ) -> anyhow::Result<ArcEscaper> {
        let escape_logger = config.get_escape_logger();

        let tls_config = if let Some(builder) = &config.tls_config {
            let tls_config = builder
                .build()
                .context("failed to setup tls client config")?;
            Some(Arc::new(tls_config))
        } else {
            None
        };

        let config = Arc::new(config);

        let peers = match peers {
            Some(peers) => peers,
            None => {
                let peers =
                    source::load_cached_peers(&config, &stats, &escape_logger, tls_config.as_ref())
                        .await
                        .unwrap_or_else(|e| {
                            warn!(
                                "failed to load cached peers for escaper {}: {e:?}",
                                config.name
                            );
                            PeerSet::default()
                        });
                Arc::new(peers)
            }
        };
        let peers = Arc::new(ArcSwap::new(peers));
        let source_job_handler = source::new_job(
            Arc::clone(&config),
            Arc::clone(&stats),
            escape_logger.clone(),
            Arc::clone(&peers),
            tls_config.clone(),
        )?;

        stats.set_extra_tags(config.extra_metrics_tags.clone());

        let escaper = ProxyFloatEscaper {
            config,
            stats,
            source_job_handler: Some(source_job_handler),
            peers,
            tls_config,
            escape_logger,
        };

        Ok(Arc::new(escaper))
    }

    pub(super) async fn prepare_initial(
        config: ProxyFloatEscaperConfig,
    ) -> anyhow::Result<ArcEscaper> {
        let stats = Arc::new(ProxyFloatEscaperStats::new(config.name()));
        ProxyFloatEscaper::new_obj(config, stats, None).await
    }

    async fn prepare_reload(
        config: AnyEscaperConfig,
        stats: Arc<ProxyFloatEscaperStats>,
        peers: Arc<PeerSet>,
    ) -> anyhow::Result<ArcEscaper> {
        if let AnyEscaperConfig::ProxyFloat(config) = config {
            ProxyFloatEscaper::new_obj(config, stats, Some(peers)).await
        } else {
            Err(anyhow!("invalid escaper config type"))
        }
    }

    fn select_peer_from_escaper(&self) -> Option<ArcNextProxyPeer> {
        let peer_set = self.peers.load();
        peer_set.select_random_peer()
    }

    fn select_peer_from_egress_path(&self, value: &Value) -> anyhow::Result<ArcNextProxyPeer> {
        let peer =
            match value {
                Value::Array(seq) => {
                    let Some(v) = seq.first() else {
                        return Err(anyhow!("empty peer array in egress path"));
                    };
                    if let Value::Object(_) = v {
                        let peer_set = peer::parse_peers(
                            &self.config,
                            &self.stats,
                            &self.escape_logger,
                            seq,
                            self.tls_config.as_ref(),
                        )?;
                        peer_set.select_random_peer()
                    } else {
                        let peer_set = self.peers.load();
                        seq.iter()
                            .filter_map(|v| {
                                let Value::String(s) = v else {
                                    return None;
                                };
                                let peer = peer_set.select_named_peer(s)?;
                                if peer.is_expired() {
                                    None
                                } else {
                                    Some(peer)
                                }
                            })
                            .choose(&mut rand::thread_rng())
                    }
                }
                Value::Object(_) => {
                    let peer = peer::parse_peer(
                        &self.config,
                        &self.stats,
                        &self.escape_logger,
                        value,
                        self.tls_config.as_ref(),
                    )?;
                    peer.and_then(|v| if v.is_expired() { None } else { Some(v) })
                }
                Value::String(id) => {
                    let peer_set = self.peers.load();
                    peer_set.select_named_peer(id).and_then(|v| {
                        if v.is_expired() {
                            None
                        } else {
                            Some(v)
                        }
                    })
                }
                _ => return Err(anyhow!("unsupported json value type for peer selection")),
            };
        peer.ok_or_else(|| anyhow!("no peer available in egress path"))
    }

    fn select_peer(&self, task_notes: &ServerTaskNotes) -> anyhow::Result<ArcNextProxyPeer> {
        if let Some(v) = task_notes
            .egress_path_selection
            .select_json_value_by_key(self.name().as_str())
        {
            self.select_peer_from_egress_path(v)
                .context("failed to select peer from egress path")
        } else {
            self.select_peer_from_escaper()
                .ok_or_else(|| anyhow!("no peer can be selected from escaper config"))
        }
    }
}

#[async_trait]
impl Escaper for ProxyFloatEscaper {
    fn name(&self) -> &MetricsName {
        self.config.name()
    }

    fn escaper_type(&self) -> &str {
        self.config.escaper_type()
    }

    fn get_escape_stats(&self) -> Option<ArcEscaperStats> {
        Some(Arc::clone(&self.stats) as _)
    }

    async fn publish(&self, data: String) -> anyhow::Result<()> {
        source::publish_peers(
            &self.config,
            &self.stats,
            &self.escape_logger,
            &self.peers,
            self.tls_config.as_ref(),
            data,
        )
        .await
    }

    async fn tcp_setup_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
    ) -> TcpConnectResult {
        self.stats.interface.add_tcp_connect_attempted();
        tcp_notes.escaper.clone_from(&self.config.name);
        let peer = self
            .select_peer(task_notes)
            .map_err(TcpConnectError::EscaperNotUsable)?;
        peer.tcp_setup_connection(tcp_notes, task_notes, task_stats)
            .await
    }

    async fn tls_setup_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
        tls_config: &'a OpensslClientConfig,
        tls_name: &'a str,
    ) -> TcpConnectResult {
        self.stats.interface.add_tls_connect_attempted();
        tcp_notes.escaper.clone_from(&self.config.name);
        let peer = self
            .select_peer(task_notes)
            .map_err(TcpConnectError::EscaperNotUsable)?;
        peer.tls_setup_connection(tcp_notes, task_notes, task_stats, tls_config, tls_name)
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
        let peer = self
            .select_peer(task_notes)
            .map_err(UdpConnectError::EscaperNotUsable)?;
        peer.udp_setup_connection(udp_notes, task_notes, task_stats)
            .await
    }

    async fn udp_setup_relay<'a>(
        &'a self,
        udp_notes: &'a mut UdpRelayTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcUdpRelayTaskRemoteStats,
    ) -> UdpRelaySetupResult {
        self.stats.interface.add_udp_relay_session_attempted();
        udp_notes.escaper.clone_from(&self.config.name);
        let peer = self
            .select_peer(task_notes)
            .map_err(UdpRelaySetupError::EscaperNotUsable)?;
        peer.udp_setup_relay(udp_notes, task_notes, task_stats)
            .await
    }

    fn new_http_forward_context(&self, escaper: ArcEscaper) -> BoxHttpForwardContext {
        let ctx = DirectHttpForwardContext::new(Arc::clone(&self.stats) as _, escaper);
        Box::new(ctx)
    }

    async fn new_ftp_connect_context<'a>(
        &'a self,
        _escaper: ArcEscaper,
        _task_notes: &'a ServerTaskNotes,
        _upstream: &'a UpstreamAddr,
    ) -> BoxFtpConnectContext {
        Box::new(DenyFtpConnectContext::new(self.name(), None))
    }
}

#[async_trait]
impl EscaperInternal for ProxyFloatEscaper {
    fn _resolver(&self) -> &MetricsName {
        Default::default()
    }

    fn _dependent_escaper(&self) -> Option<BTreeSet<MetricsName>> {
        None
    }

    fn _clone_config(&self) -> AnyEscaperConfig {
        let config = &*self.config;
        AnyEscaperConfig::ProxyFloat(config.clone())
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
        // copy the old peers, they may be a little outdated at this stage
        // as we haven't stop the old job
        let peers = self.peers.load_full();
        ProxyFloatEscaper::prepare_reload(config, stats, peers).await
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
        let peer = self
            .select_peer(task_notes)
            .map_err(TcpConnectError::EscaperNotUsable)?;
        peer.new_http_forward_connection(tcp_notes, task_notes, task_stats)
            .await
    }

    async fn _new_https_forward_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        task_notes: &'a ServerTaskNotes,
        task_stats: ArcHttpForwardTaskRemoteStats,
        tls_config: &'a OpensslClientConfig,
        tls_name: &'a str,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        self.stats
            .interface
            .add_https_forward_connection_attempted();
        tcp_notes.escaper.clone_from(&self.config.name);
        let peer = self
            .select_peer(task_notes)
            .map_err(TcpConnectError::EscaperNotUsable)?;
        peer.new_https_forward_connection(tcp_notes, task_notes, task_stats, tls_config, tls_name)
            .await
    }

    async fn _new_ftp_control_connection<'a>(
        &'a self,
        tcp_notes: &'a mut TcpConnectTaskNotes,
        _task_notes: &'a ServerTaskNotes,
        _task_stats: ArcFtpTaskRemoteControlStats,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError> {
        self.stats.interface.add_ftp_over_http_request_attempted();
        self.stats.interface.add_ftp_control_connection_attempted();
        tcp_notes.escaper.clone_from(&self.config.name);
        Err(TcpConnectError::MethodUnavailable)
    }

    async fn _new_ftp_transfer_connection<'a>(
        &'a self,
        transfer_tcp_notes: &'a mut TcpConnectTaskNotes,
        _control_tcp_notes: &'a TcpConnectTaskNotes,
        _task_notes: &'a ServerTaskNotes,
        _task_stats: ArcFtpTaskRemoteTransferStats,
        _context: AnyFtpConnectContextParam,
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
