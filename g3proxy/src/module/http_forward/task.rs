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

use http::{Method, Uri};
use tokio::time::{Duration, Instant};

pub(crate) struct HttpForwardTaskNotes {
    pub(crate) method: Method,
    pub(crate) uri: Uri,
    pub(crate) uri_log_max_chars: usize,
    pub(crate) rsp_status: u16,
    pub(crate) origin_status: u16,
    pub(crate) pipeline_wait: Duration,
    pub(crate) reuse_connection: bool,
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
            reuse_connection: false,
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
