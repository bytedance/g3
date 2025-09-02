/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use chrono::{DateTime, Utc};
use tokio::time::Instant;
use uuid::Uuid;

use g3_daemon::server::ClientConnectionInfo;
use g3_types::limit::GaugeSemaphorePermit;
use g3_types::net::UpstreamAddr;

use crate::auth::UserContext;
use crate::escape::EgressPathSelection;

#[derive(Clone, Copy)]
pub(crate) enum ServerTaskStage {
    Created,
    Preparing,
    Connecting,
    Connected,
    Replying,
    LoggedIn,
    Relaying,
    Finished,
}

impl ServerTaskStage {
    pub(crate) fn brief(&self) -> &'static str {
        match self {
            ServerTaskStage::Created => "Created",
            ServerTaskStage::Preparing => "Preparing",
            ServerTaskStage::Connecting => "Connecting",
            ServerTaskStage::Connected => "Connected",
            ServerTaskStage::Replying => "Replying",
            ServerTaskStage::LoggedIn => "LoggedIn",
            ServerTaskStage::Relaying => "Relaying",
            ServerTaskStage::Finished => "Finished",
        }
    }
}

/// server task notes is bounded to a single client connection.
/// it can be reset if the connection is consisted of many tasks.
/// Do not share this struct between different client connections.
pub(crate) struct ServerTaskNotes {
    cc_info: ClientConnectionInfo,
    pub(crate) stage: ServerTaskStage,
    pub(crate) start_at: DateTime<Utc>,
    create_ins: Instant,
    pub(crate) id: Uuid,
    user_ctx: Option<UserContext>,
    pub(crate) wait_time: Duration,
    pub(crate) ready_time: Duration,
    pub(crate) egress_path_selection: Option<EgressPathSelection>,
    // optional, per-connection override for next-hop proxy (escaper peer)
    override_next_proxy: Option<UpstreamAddr>,
    /// the following fields should not be cloned
    pub(crate) user_req_alive_permit: Option<GaugeSemaphorePermit>,
}

impl ServerTaskNotes {
    pub(crate) fn new(
        cc_info: ClientConnectionInfo,
        user_ctx: Option<UserContext>,
        wait_time: Duration,
    ) -> Self {
        ServerTaskNotes::with_path_selection(cc_info, user_ctx, wait_time, None)
    }

    pub(crate) fn with_path_selection(
        cc_info: ClientConnectionInfo,
        user_ctx: Option<UserContext>,
        wait_time: Duration,
        egress_path_selection: Option<EgressPathSelection>,
    ) -> Self {
        let started = Utc::now();
        let uuid = g3_daemon::server::task::generate_uuid(&started);
        ServerTaskNotes {
            cc_info,
            stage: ServerTaskStage::Created,
            start_at: started,
            create_ins: Instant::now(),
            id: uuid,
            user_ctx,
            wait_time,
            ready_time: Duration::default(),
            egress_path_selection,
            override_next_proxy: None,
            user_req_alive_permit: None,
        }
    }

    #[inline]
    pub(crate) fn client_addr(&self) -> SocketAddr {
        self.cc_info.client_addr()
    }

    #[inline]
    pub(crate) fn client_ip(&self) -> IpAddr {
        self.cc_info.client_ip()
    }

    #[inline]
    pub(crate) fn server_addr(&self) -> SocketAddr {
        self.cc_info.server_addr()
    }

    #[inline]
    pub(crate) fn worker_id(&self) -> Option<usize> {
        self.cc_info.worker_id()
    }

    #[inline]
    pub(crate) fn user_ctx(&self) -> Option<&UserContext> {
        self.user_ctx.as_ref()
    }

    #[inline]
    pub(crate) fn user_ctx_mut(&mut self) -> Option<&mut UserContext> {
        self.user_ctx.as_mut()
    }

    pub(crate) fn raw_user_name(&self) -> Option<&Arc<str>> {
        self.user_ctx.as_ref().and_then(|c| c.raw_user_name())
    }

    pub(crate) fn egress_path(&self) -> Option<&EgressPathSelection> {
        self.user_ctx
            .as_ref()
            .and_then(|ctx| ctx.user_config().egress_path_selection.as_ref())
            .or(self.egress_path_selection.as_ref())
    }

    // Username-derived escaper override helpers
    #[inline]
    pub(crate) fn set_override_next_proxy(&mut self, addr: UpstreamAddr) {
        self.override_next_proxy = Some(addr);
    }

    #[inline]
    pub(crate) fn override_next_proxy(&self) -> Option<&UpstreamAddr> {
        self.override_next_proxy.as_ref()
    }

    #[inline]
    pub(crate) fn task_created_instant(&self) -> Instant {
        self.create_ins
    }

    #[inline]
    pub(crate) fn time_elapsed(&self) -> Duration {
        self.create_ins.elapsed()
    }

    pub(crate) fn mark_relaying(&mut self) {
        self.stage = ServerTaskStage::Relaying;
        self.ready_time = self.create_ins.elapsed();
        if let Some(user_ctx) = &self.user_ctx {
            user_ctx.record_task_ready(self.ready_time);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use g3_daemon::server::ClientConnectionInfo;
    use g3_types::net::{Host, UpstreamAddr};

    #[test]
    fn override_next_proxy_helpers() {
        let cc = ClientConnectionInfo::new(
            "127.0.0.1:10000".parse().unwrap(),
            "127.0.0.1:20000".parse().unwrap(),
        );
        let mut notes = ServerTaskNotes::new(cc, None, Duration::from_secs(0));
        assert!(notes.override_next_proxy().is_none());

        let addr = UpstreamAddr::from_host_str_and_port("127.0.0.1", 8080).unwrap();
        notes.set_override_next_proxy(addr.clone());
        let got = notes.override_next_proxy().unwrap();
        assert_eq!(got.port(), 8080);
        match got.host() {
            Host::Ip(ip) => assert_eq!(ip.to_string(), "127.0.0.1"),
            _ => panic!("expected ip host"),
        }
    }
}
