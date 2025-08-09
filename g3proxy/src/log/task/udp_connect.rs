/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;

use slog::{Logger, slog_info};

use g3_slog_types::{LtDateTime, LtDuration, LtIpAddr, LtUpstreamAddr, LtUuid};
use g3_types::net::UpstreamAddr;

use super::TaskEvent;
use crate::module::udp_connect::UdpConnectTaskNotes;
use crate::serve::{ServerTaskError, ServerTaskNotes};

pub(crate) struct TaskLogForUdpConnect<'a> {
    pub(crate) logger: &'a Logger,
    pub(crate) task_notes: &'a ServerTaskNotes,
    pub(crate) tcp_server_addr: SocketAddr,
    pub(crate) tcp_client_addr: SocketAddr,
    pub(crate) udp_listen_addr: Option<SocketAddr>,
    pub(crate) udp_client_addr: Option<SocketAddr>,
    pub(crate) upstream: Option<&'a UpstreamAddr>,
    pub(crate) udp_notes: &'a UdpConnectTaskNotes,
    pub(crate) client_rd_bytes: u64,
    pub(crate) client_rd_packets: u64,
    pub(crate) client_wr_bytes: u64,
    pub(crate) client_wr_packets: u64,
    pub(crate) remote_rd_bytes: u64,
    pub(crate) remote_rd_packets: u64,
    pub(crate) remote_wr_bytes: u64,
    pub(crate) remote_wr_packets: u64,
}

impl TaskLogForUdpConnect<'_> {
    pub(crate) fn log_created(&self) {
        if let Some(user_ctx) = self.task_notes.user_ctx()
            && user_ctx.skip_log()
        {
            return;
        }

        slog_info!(self.logger, "";
            "task_type" => "UdpConnect",
            "task_id" => LtUuid(&self.task_notes.id),
            "task_event" => TaskEvent::Created.as_str(),
            "stage" => self.task_notes.stage.brief(),
            "start_at" => LtDateTime(&self.task_notes.start_at),
            "user" => self.task_notes.raw_user_name(),
            "tcp_server_addr" => self.tcp_server_addr,
            "tcp_client_addr" => self.tcp_client_addr,
            "wait_time" => LtDuration(self.task_notes.wait_time),
        )
    }

    pub(crate) fn log_connected(&self) {
        if let Some(user_ctx) = self.task_notes.user_ctx()
            && user_ctx.skip_log()
        {
            return;
        }

        slog_info!(self.logger, "";
            "task_type" => "UdpConnect",
            "task_id" => LtUuid(&self.task_notes.id),
            "task_event" => TaskEvent::Connected.as_str(),
            "stage" => self.task_notes.stage.brief(),
            "start_at" => LtDateTime(&self.task_notes.start_at),
            "user" => self.task_notes.raw_user_name(),
            "tcp_server_addr" => self.tcp_server_addr,
            "tcp_client_addr" => self.tcp_client_addr,
            "udp_listen_addr" => self.udp_listen_addr,
            "udp_client_addr" => self.udp_client_addr,
            "upstream" => self.upstream.map(LtUpstreamAddr),
            "escaper" => self.udp_notes.escaper.as_str(),
            "next_bind_ip" => self.udp_notes.bind.ip().map(LtIpAddr),
            "next_bound_addr" => self.udp_notes.local,
            "next_peer_addr" => self.udp_notes.next,
            "next_expire" => self.udp_notes.expire.as_ref().map(LtDateTime),
            "wait_time" => LtDuration(self.task_notes.wait_time),
            "ready_time" => LtDuration(self.task_notes.ready_time),
            "c_rd_bytes" => self.client_rd_bytes,
            "c_rd_packets" => self.client_rd_packets,
        )
    }

    pub(crate) fn log_periodic(&self) {
        if let Some(user_ctx) = self.task_notes.user_ctx()
            && user_ctx.skip_log()
        {
            return;
        }

        slog_info!(self.logger, "";
            "task_type" => "UdpConnect",
            "task_id" => LtUuid(&self.task_notes.id),
            "task_event" => TaskEvent::Periodic.as_str(),
            "stage" => self.task_notes.stage.brief(),
            "start_at" => LtDateTime(&self.task_notes.start_at),
            "user" => self.task_notes.raw_user_name(),
            "tcp_server_addr" => self.tcp_server_addr,
            "tcp_client_addr" => self.tcp_client_addr,
            "udp_listen_addr" => self.udp_listen_addr,
            "udp_client_addr" => self.udp_client_addr,
            "upstream" => self.upstream.map(LtUpstreamAddr),
            "escaper" => self.udp_notes.escaper.as_str(),
            "next_bind_ip" => self.udp_notes.bind.ip().map(LtIpAddr),
            "next_bound_addr" => self.udp_notes.local,
            "next_peer_addr" => self.udp_notes.next,
            "next_expire" => self.udp_notes.expire.as_ref().map(LtDateTime),
            "wait_time" => LtDuration(self.task_notes.wait_time),
            "ready_time" => LtDuration(self.task_notes.ready_time),
            "total_time" => LtDuration(self.task_notes.time_elapsed()),
            "c_rd_bytes" => self.client_rd_bytes,
            "c_rd_packets" => self.client_rd_packets,
            "c_wr_bytes" => self.client_wr_bytes,
            "c_wr_packets" => self.client_wr_packets,
            "r_rd_bytes" => self.remote_rd_bytes,
            "r_rd_packets" => self.remote_rd_packets,
            "r_wr_bytes" => self.remote_wr_bytes,
            "r_wr_packets" => self.remote_wr_packets,
        )
    }

    pub(crate) fn log(&self, e: ServerTaskError) {
        if let Some(user_ctx) = self.task_notes.user_ctx()
            && user_ctx.skip_log()
        {
            return;
        }

        slog_info!(self.logger, "{}", e;
            "task_type" => "UdpConnect",
            "task_id" => LtUuid(&self.task_notes.id),
            "task_event" => TaskEvent::Finished.as_str(),
            "stage" => self.task_notes.stage.brief(),
            "start_at" => LtDateTime(&self.task_notes.start_at),
            "user" => self.task_notes.raw_user_name(),
            "tcp_server_addr" => self.tcp_server_addr,
            "tcp_client_addr" => self.tcp_client_addr,
            "udp_listen_addr" => self.udp_listen_addr,
            "udp_client_addr" => self.udp_client_addr,
            "upstream" => self.upstream.map(LtUpstreamAddr),
            "escaper" => self.udp_notes.escaper.as_str(),
            "next_bind_ip" => self.udp_notes.bind.ip().map(LtIpAddr),
            "next_bound_addr" => self.udp_notes.local,
            "next_peer_addr" => self.udp_notes.next,
            "next_expire" => self.udp_notes.expire.as_ref().map(LtDateTime),
            "reason" => e.brief(),
            "wait_time" => LtDuration(self.task_notes.wait_time),
            "ready_time" => LtDuration(self.task_notes.ready_time),
            "total_time" => LtDuration(self.task_notes.time_elapsed()),
            "c_rd_bytes" => self.client_rd_bytes,
            "c_rd_packets" => self.client_rd_packets,
            "c_wr_bytes" => self.client_wr_bytes,
            "c_wr_packets" => self.client_wr_packets,
            "r_rd_bytes" => self.remote_rd_bytes,
            "r_rd_packets" => self.remote_rd_packets,
            "r_wr_bytes" => self.remote_wr_bytes,
            "r_wr_packets" => self.remote_wr_packets,
        )
    }
}
