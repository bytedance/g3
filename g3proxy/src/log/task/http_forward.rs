/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use slog::{Logger, slog_info};

use g3_slog_types::{
    LtDateTime, LtDuration, LtHttpMethod, LtHttpUri, LtIpAddr, LtUpstreamAddr, LtUuid,
};
use g3_types::net::UpstreamAddr;

use super::TaskEvent;
use crate::module::http_forward::HttpForwardTaskNotes;
use crate::module::tcp_connect::TcpConnectTaskNotes;
use crate::serve::{ServerTaskError, ServerTaskNotes};

pub(crate) struct TaskLogForHttpForward<'a> {
    pub(crate) logger: &'a Logger,
    pub(crate) upstream: &'a UpstreamAddr,
    pub(crate) task_notes: &'a ServerTaskNotes,
    pub(crate) http_notes: &'a HttpForwardTaskNotes,
    pub(crate) http_user_agent: Option<&'a str>,
    pub(crate) tcp_notes: &'a TcpConnectTaskNotes,
    pub(crate) client_rd_bytes: u64,
    pub(crate) client_wr_bytes: u64,
    pub(crate) remote_rd_bytes: u64,
    pub(crate) remote_wr_bytes: u64,
}

impl TaskLogForHttpForward<'_> {
    pub(crate) fn log_created(&self) {
        if let Some(user_ctx) = self.task_notes.user_ctx() {
            if user_ctx.skip_log() {
                return;
            }
        }

        slog_info!(self.logger, "";
            "task_type" => "HttpForward",
            "task_id" => LtUuid(&self.task_notes.id),
            "task_event" => TaskEvent::Created.as_str(),
            "stage" => self.task_notes.stage.brief(),
            "start_at" => LtDateTime(&self.task_notes.start_at),
            "user" => self.task_notes.raw_user_name(),
            "server_addr" => self.task_notes.server_addr(),
            "client_addr" => self.task_notes.client_addr(),
            "upstream" => LtUpstreamAddr(self.upstream),
            "pipeline_wait" => LtDuration(self.http_notes.pipeline_wait),
            "method" => LtHttpMethod(&self.http_notes.method),
            "uri" => LtHttpUri::new(&self.http_notes.uri, self.http_notes.uri_log_max_chars),
            "user_agent" => self.http_user_agent,
            "wait_time" => LtDuration(self.task_notes.wait_time),
        )
    }

    pub(crate) fn log_connected(&self) {
        if let Some(user_ctx) = self.task_notes.user_ctx() {
            if user_ctx.skip_log() {
                return;
            }
        }

        slog_info!(self.logger, "";
            "task_type" => "HttpForward",
            "task_id" => LtUuid(&self.task_notes.id),
            "task_event" => TaskEvent::Connected.as_str(),
            "stage" => self.task_notes.stage.brief(),
            "start_at" => LtDateTime(&self.task_notes.start_at),
            "user" => self.task_notes.raw_user_name(),
            "server_addr" => self.task_notes.server_addr(),
            "client_addr" => self.task_notes.client_addr(),
            "upstream" => LtUpstreamAddr(self.upstream),
            "escaper" => self.tcp_notes.escaper.as_str(),
            "next_bind_ip" => self.tcp_notes.bind.ip().map(LtIpAddr),
            "next_bound_addr" => self.tcp_notes.local,
            "next_peer_addr" => self.tcp_notes.next,
            "next_expire" => self.tcp_notes.expire.as_ref().map(LtDateTime),
            "tcp_connect_tries" => self.tcp_notes.tries,
            "tcp_connect_spend" => LtDuration(self.tcp_notes.duration),
            "pipeline_wait" => LtDuration(self.http_notes.pipeline_wait),
            "reuse_connection" => self.http_notes.reused_connection,
            "method" => LtHttpMethod(&self.http_notes.method),
            "uri" => LtHttpUri::new(&self.http_notes.uri, self.http_notes.uri_log_max_chars),
            "user_agent" => self.http_user_agent,
            "wait_time" => LtDuration(self.task_notes.wait_time),
            "ready_time" => LtDuration(self.task_notes.ready_time),
        )
    }

    pub(crate) fn log_periodic(&self) {
        if let Some(user_ctx) = self.task_notes.user_ctx() {
            if user_ctx.skip_log() {
                return;
            }
        }

        slog_info!(self.logger, "";
            "task_type" => "HttpForward",
            "task_id" => LtUuid(&self.task_notes.id),
            "task_event" => TaskEvent::Periodic.as_str(),
            "stage" => self.task_notes.stage.brief(),
            "start_at" => LtDateTime(&self.task_notes.start_at),
            "user" => self.task_notes.raw_user_name(),
            "server_addr" => self.task_notes.server_addr(),
            "client_addr" => self.task_notes.client_addr(),
            "upstream" => LtUpstreamAddr(self.upstream),
            "escaper" => self.tcp_notes.escaper.as_str(),
            "next_bind_ip" => self.tcp_notes.bind.ip().map(LtIpAddr),
            "next_bound_addr" => self.tcp_notes.local,
            "next_peer_addr" => self.tcp_notes.next,
            "next_expire" => self.tcp_notes.expire.as_ref().map(LtDateTime),
            "tcp_connect_tries" => self.tcp_notes.tries,
            "tcp_connect_spend" => LtDuration(self.tcp_notes.duration),
            "pipeline_wait" => LtDuration(self.http_notes.pipeline_wait),
            "reuse_connection" => self.http_notes.reused_connection,
            "method" => LtHttpMethod(&self.http_notes.method),
            "uri" => LtHttpUri::new(&self.http_notes.uri, self.http_notes.uri_log_max_chars),
            "user_agent" => self.http_user_agent,
            "rsp_status" => self.http_notes.rsp_status,
            "origin_status" => self.http_notes.origin_status,
            "wait_time" => LtDuration(self.task_notes.wait_time),
            "ready_time" => LtDuration(self.task_notes.ready_time),
            "total_time" => LtDuration(self.task_notes.time_elapsed()),
            "dur_req_send_hdr" => LtDuration(self.http_notes.dur_req_send_hdr),
            "dur_req_send_all" => LtDuration(self.http_notes.dur_req_send_all),
            "dur_rsp_recv_hdr" => LtDuration(self.http_notes.dur_rsp_recv_hdr),
            "dur_rsp_recv_all" => LtDuration(self.http_notes.dur_rsp_recv_all),
            "c_rd_bytes" => self.client_rd_bytes,
            "c_wr_bytes" => self.client_wr_bytes,
            "r_rd_bytes" => self.remote_rd_bytes,
            "r_wr_bytes" => self.remote_wr_bytes,
        )
    }

    pub(crate) fn log(&self, e: &ServerTaskError) {
        if let Some(user_ctx) = self.task_notes.user_ctx() {
            if user_ctx.skip_log() {
                return;
            }
        }

        slog_info!(self.logger, "{}", e;
            "task_type" => "HttpForward",
            "task_id" => LtUuid(&self.task_notes.id),
            "task_event" => TaskEvent::Finished.as_str(),
            "stage" => self.task_notes.stage.brief(),
            "start_at" => LtDateTime(&self.task_notes.start_at),
            "user" => self.task_notes.raw_user_name(),
            "server_addr" => self.task_notes.server_addr(),
            "client_addr" => self.task_notes.client_addr(),
            "upstream" => LtUpstreamAddr(self.upstream),
            "escaper" => self.tcp_notes.escaper.as_str(),
            "next_bind_ip" => self.tcp_notes.bind.ip().map(LtIpAddr),
            "next_bound_addr" => self.tcp_notes.local,
            "next_peer_addr" => self.tcp_notes.next,
            "next_expire" => self.tcp_notes.expire.as_ref().map(LtDateTime),
            "tcp_connect_tries" => self.tcp_notes.tries,
            "tcp_connect_spend" => LtDuration(self.tcp_notes.duration),
            "reason" => e.brief(),
            "pipeline_wait" => LtDuration(self.http_notes.pipeline_wait),
            "reuse_connection" => self.http_notes.reused_connection,
            "method" => LtHttpMethod(&self.http_notes.method),
            "uri" => LtHttpUri::new(&self.http_notes.uri, self.http_notes.uri_log_max_chars),
            "user_agent" => self.http_user_agent,
            "rsp_status" => self.http_notes.rsp_status,
            "origin_status" => self.http_notes.origin_status,
            "wait_time" => LtDuration(self.task_notes.wait_time),
            "ready_time" => LtDuration(self.task_notes.ready_time),
            "dur_req_send_hdr" => LtDuration(self.http_notes.dur_req_send_hdr),
            "dur_req_send_all" => LtDuration(self.http_notes.dur_req_send_all),
            "dur_rsp_recv_hdr" => LtDuration(self.http_notes.dur_rsp_recv_hdr),
            "dur_rsp_recv_all" => LtDuration(self.http_notes.dur_rsp_recv_all),
            "total_time" => LtDuration(self.task_notes.time_elapsed()),
            "c_rd_bytes" => self.client_rd_bytes,
            "c_wr_bytes" => self.client_wr_bytes,
            "r_rd_bytes" => self.remote_rd_bytes,
            "r_wr_bytes" => self.remote_wr_bytes,
        )
    }
}
