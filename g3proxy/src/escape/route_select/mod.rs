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
use std::hash::{Hash, Hasher};
use std::sync::Arc;

use anyhow::anyhow;
use async_trait::async_trait;
use foldhash::{HashMap, HashMapExt};

use g3_daemon::stat::remote::ArcTcpConnectionTaskRemoteStats;
use g3_types::collection::{SelectiveVec, SelectiveVecBuilder, WeightedValue};
use g3_types::metrics::NodeName;
use g3_types::net::UpstreamAddr;

use super::{ArcEscaper, Escaper, EscaperExt, EscaperInternal, RouteEscaperStats};
use crate::audit::AuditContext;
use crate::config::escaper::route_select::RouteSelectEscaperConfig;
use crate::config::escaper::{AnyEscaperConfig, EscaperConfig};
use crate::module::ftp_over_http::{
    ArcFtpTaskRemoteControlStats, ArcFtpTaskRemoteTransferStats, BoxFtpConnectContext,
    BoxFtpRemoteConnection, DenyFtpConnectContext,
};
use crate::module::http_forward::{
    ArcHttpForwardTaskRemoteStats, BoxHttpForwardConnection, BoxHttpForwardContext,
    RouteHttpForwardContext,
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

struct EscaperWrapper {
    escaper: ArcEscaper,
}

impl Hash for EscaperWrapper {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.escaper.name().hash(state);
    }
}

pub(super) struct RouteSelectEscaper {
    config: RouteSelectEscaperConfig,
    stats: Arc<RouteEscaperStats>,
    all_nodes: HashMap<NodeName, ArcEscaper>,
    select_nodes: SelectiveVec<WeightedValue<EscaperWrapper>>,
}

impl RouteSelectEscaper {
    fn new_obj(
        config: RouteSelectEscaperConfig,
        stats: Arc<RouteEscaperStats>,
    ) -> anyhow::Result<ArcEscaper> {
        let mut all_nodes = HashMap::with_capacity(config.next_nodes.len());
        let mut select_nodes_builder = SelectiveVecBuilder::with_capacity(config.next_nodes.len());
        for v in &config.next_nodes {
            let escaper = super::registry::get_or_insert_default(v.inner());
            all_nodes.insert(escaper.name().clone(), escaper.clone());
            if v.weight() > 0f64 {
                select_nodes_builder.insert(WeightedValue::with_weight(
                    EscaperWrapper { escaper },
                    v.weight(),
                ));
            }
        }

        let select_nodes = select_nodes_builder
            .build()
            .ok_or_else(|| anyhow!("no next escaper set"))?;

        let escaper = RouteSelectEscaper {
            config,
            stats,
            all_nodes,
            select_nodes,
        };

        Ok(Arc::new(escaper))
    }

    pub(super) fn prepare_initial(config: RouteSelectEscaperConfig) -> anyhow::Result<ArcEscaper> {
        let stats = Arc::new(RouteEscaperStats::new(config.name()));
        RouteSelectEscaper::new_obj(config, stats)
    }

    fn prepare_reload(
        config: AnyEscaperConfig,
        stats: Arc<RouteEscaperStats>,
    ) -> anyhow::Result<ArcEscaper> {
        if let AnyEscaperConfig::RouteSelect(config) = config {
            RouteSelectEscaper::new_obj(config, stats)
        } else {
            Err(anyhow!("invalid escaper config type"))
        }
    }

    fn select_next(
        &self,
        task_notes: &ServerTaskNotes,
        upstream: &UpstreamAddr,
    ) -> anyhow::Result<ArcEscaper> {
        if let Some(path_selection) = task_notes.egress_path() {
            if let Some(id) = path_selection.select_matched_id(self.name().as_str()) {
                return self
                    .all_nodes
                    .get(id)
                    .cloned()
                    .ok_or_else(|| anyhow!("no next escaper {id} found in local cache"));
            }
        }

        let v = self.select_consistent(
            &self.select_nodes,
            self.config.next_pick_policy,
            task_notes,
            upstream.host(),
        );
        Ok(v.inner().escaper.clone())
    }
}

impl EscaperExt for RouteSelectEscaper {}

#[async_trait]
impl Escaper for RouteSelectEscaper {
    fn name(&self) -> &NodeName {
        self.config.name()
    }

    fn escaper_type(&self) -> &str {
        self.config.escaper_type()
    }

    fn ref_route_stats(&self) -> Option<&Arc<RouteEscaperStats>> {
        Some(&self.stats)
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
        audit_ctx: &mut AuditContext,
    ) -> TcpConnectResult {
        tcp_notes.escaper.clone_from(&self.config.name);
        match self.select_next(task_notes, task_conf.upstream) {
            Ok(escaper) => {
                self.stats.add_request_passed();
                escaper
                    .tcp_setup_connection(task_conf, tcp_notes, task_notes, task_stats, audit_ctx)
                    .await
            }
            Err(e) => {
                self.stats.add_request_failed();
                Err(TcpConnectError::EscaperNotUsable(e))
            }
        }
    }

    async fn tls_setup_connection(
        &self,
        task_conf: &TlsConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcTcpConnectionTaskRemoteStats,
        audit_ctx: &mut AuditContext,
    ) -> TcpConnectResult {
        tcp_notes.escaper.clone_from(&self.config.name);
        match self.select_next(task_notes, task_conf.tcp.upstream) {
            Ok(escaper) => {
                self.stats.add_request_passed();
                escaper
                    .tls_setup_connection(task_conf, tcp_notes, task_notes, task_stats, audit_ctx)
                    .await
            }
            Err(e) => {
                self.stats.add_request_failed();
                Err(TcpConnectError::EscaperNotUsable(e))
            }
        }
    }

    async fn udp_setup_connection(
        &self,
        task_conf: &UdpConnectTaskConf<'_>,
        udp_notes: &mut UdpConnectTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcUdpConnectTaskRemoteStats,
    ) -> UdpConnectResult {
        udp_notes.escaper.clone_from(&self.config.name);
        match self.select_next(task_notes, task_conf.upstream) {
            Ok(escaper) => {
                self.stats.add_request_passed();
                escaper
                    .udp_setup_connection(task_conf, udp_notes, task_notes, task_stats)
                    .await
            }
            Err(e) => {
                self.stats.add_request_failed();
                Err(UdpConnectError::EscaperNotUsable(e))
            }
        }
    }

    async fn udp_setup_relay(
        &self,
        task_conf: &UdpRelayTaskConf<'_>,
        udp_notes: &mut UdpRelayTaskNotes,
        task_notes: &ServerTaskNotes,
        task_stats: ArcUdpRelayTaskRemoteStats,
    ) -> UdpRelaySetupResult {
        udp_notes.escaper.clone_from(&self.config.name);
        match self.select_next(task_notes, task_conf.initial_peer) {
            Ok(escaper) => {
                self.stats.add_request_passed();
                escaper
                    .udp_setup_relay(task_conf, udp_notes, task_notes, task_stats)
                    .await
            }
            Err(e) => {
                self.stats.add_request_failed();
                Err(UdpRelaySetupError::EscaperNotUsable(e))
            }
        }
    }

    fn new_http_forward_context(&self, escaper: ArcEscaper) -> BoxHttpForwardContext {
        let ctx = RouteHttpForwardContext::new(escaper);
        Box::new(ctx)
    }

    async fn new_ftp_connect_context(
        &self,
        _escaper: ArcEscaper,
        task_conf: &TcpConnectTaskConf<'_>,
        task_notes: &ServerTaskNotes,
    ) -> BoxFtpConnectContext {
        match self.select_next(task_notes, task_conf.upstream) {
            Ok(escaper) => {
                self.stats.add_request_passed();
                escaper
                    .new_ftp_connect_context(Arc::clone(&escaper), task_conf, task_notes)
                    .await
            }
            Err(e) => {
                self.stats.add_request_failed();
                Box::new(DenyFtpConnectContext::new(
                    self.name(),
                    Some(TcpConnectError::EscaperNotUsable(e)),
                ))
            }
        }
    }
}

#[async_trait]
impl EscaperInternal for RouteSelectEscaper {
    fn _resolver(&self) -> &NodeName {
        Default::default()
    }

    fn _dependent_escaper(&self) -> Option<BTreeSet<NodeName>> {
        self.config.dependent_escaper()
    }

    fn _clone_config(&self) -> AnyEscaperConfig {
        AnyEscaperConfig::RouteSelect(self.config.clone())
    }

    async fn _lock_safe_reload(&self, config: AnyEscaperConfig) -> anyhow::Result<ArcEscaper> {
        let stats = Arc::clone(&self.stats);
        RouteSelectEscaper::prepare_reload(config, stats)
    }

    async fn _check_out_next_escaper(
        &self,
        task_notes: &ServerTaskNotes,
        upstream: &UpstreamAddr,
    ) -> Option<ArcEscaper> {
        match self.select_next(task_notes, upstream) {
            Ok(escaper) => {
                self.stats.add_request_passed();
                Some(escaper)
            }
            Err(_) => {
                self.stats.add_request_failed();
                None
            }
        }
    }

    async fn _new_http_forward_connection(
        &self,
        _task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        _task_notes: &ServerTaskNotes,
        _task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        tcp_notes.escaper.clone_from(&self.config.name);
        Err(TcpConnectError::MethodUnavailable)
    }

    async fn _new_https_forward_connection(
        &self,
        _task_conf: &TlsConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        _task_notes: &ServerTaskNotes,
        _task_stats: ArcHttpForwardTaskRemoteStats,
    ) -> Result<BoxHttpForwardConnection, TcpConnectError> {
        tcp_notes.escaper.clone_from(&self.config.name);
        Err(TcpConnectError::MethodUnavailable)
    }

    async fn _new_ftp_control_connection(
        &self,
        _task_conf: &TcpConnectTaskConf<'_>,
        tcp_notes: &mut TcpConnectTaskNotes,
        _task_notes: &ServerTaskNotes,
        _task_stats: ArcFtpTaskRemoteControlStats,
    ) -> Result<BoxFtpRemoteConnection, TcpConnectError> {
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
        transfer_tcp_notes.escaper.clone_from(&self.config.name);
        Err(TcpConnectError::MethodUnavailable)
    }
}
