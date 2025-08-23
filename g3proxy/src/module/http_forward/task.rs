/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use http::{Method, Uri};
use tokio::time::{Duration, Instant};

pub(crate) struct HttpForwardTaskNotes {
    pub(crate) method: Method,
    pub(crate) uri: Uri,
    pub(crate) uri_log_max_chars: usize,
    pub(crate) rsp_status: u16,
    pub(crate) origin_status: u16,
    pub(crate) pipeline_wait: Duration,
    pub(crate) reused_connection: bool,
    create_ins: Instant,
    pub(crate) dur_req_send_hdr: Duration,
    pub(crate) dur_req_send_all: Duration,
    pub(crate) dur_rsp_recv_hdr: Duration,
    pub(crate) dur_rsp_recv_all: Duration,
    pub(crate) retry_new_connection: bool,
}

impl HttpForwardTaskNotes {
    pub(crate) fn new(
        req_received: Instant,
        task_created: Instant,
        method: Method,
        uri: Uri,
        uri_log_max_chars: usize,
    ) -> Self {
        HttpForwardTaskNotes {
            method,
            uri,
            uri_log_max_chars,
            rsp_status: 0,
            origin_status: 0,
            pipeline_wait: req_received.elapsed(),
            reused_connection: false,
            create_ins: task_created,
            dur_req_send_hdr: Duration::default(),
            dur_req_send_all: Duration::default(),
            dur_rsp_recv_hdr: Duration::default(),
            dur_rsp_recv_all: Duration::default(),
            retry_new_connection: false,
        }
    }

    pub(crate) fn mark_req_send_hdr(&mut self) {
        self.dur_req_send_hdr = self.create_ins.elapsed();
    }

    pub(crate) fn mark_req_no_body(&mut self) {
        self.dur_req_send_all = self.dur_req_send_hdr;
    }

    pub(crate) fn mark_req_send_all(&mut self) {
        self.dur_req_send_all = self.create_ins.elapsed();
    }

    pub(crate) fn mark_rsp_recv_hdr(&mut self) {
        self.dur_rsp_recv_hdr = self.create_ins.elapsed();
    }

    pub(crate) fn mark_rsp_no_body(&mut self) {
        self.dur_rsp_recv_all = self.dur_rsp_recv_hdr;
    }

    pub(crate) fn mark_rsp_recv_all(&mut self) {
        self.dur_rsp_recv_all = self.create_ins.elapsed();
    }
}
