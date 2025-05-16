/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::time::Duration;

use tokio::io::AsyncBufRead;
use tokio::time::Instant;

use g3_http::HttpBodyDecodeReader;

use crate::IcapServiceClient;
use crate::reqmod::h1::HttpAdapterErrorResponse;
use crate::service::IcapClientConnection;

pub struct ReqmodAdaptationRunState {
    task_create_instant: Instant,
    pub dur_ups_send_all: Option<Duration>,
    pub clt_read_finished: bool,
    pub ups_write_finished: bool,
}

impl ReqmodAdaptationRunState {
    pub fn new(task_create_instant: Instant) -> Self {
        ReqmodAdaptationRunState {
            task_create_instant,
            dur_ups_send_all: None,
            clt_read_finished: false,
            ups_write_finished: false,
        }
    }

    pub(crate) fn mark_ups_send_all(&mut self) {
        self.dur_ups_send_all = Some(self.task_create_instant.elapsed());
        self.ups_write_finished = true;
    }
}

pub enum ReqmodAdaptationEndState {
    OriginalTransferred,
    AdaptedTransferred,
    HttpErrResponse(HttpAdapterErrorResponse, Option<ReqmodRecvHttpResponseBody>),
}

pub struct ReqmodRecvHttpResponseBody {
    pub(crate) icap_client: Arc<IcapServiceClient>,
    pub(crate) icap_keepalive: bool,
    pub(crate) icap_connection: IcapClientConnection,
}

impl ReqmodRecvHttpResponseBody {
    pub fn body_reader(&mut self) -> HttpBodyDecodeReader<'_, impl AsyncBufRead + use<>> {
        HttpBodyDecodeReader::new_chunked(&mut self.icap_connection.reader, 1024)
    }

    pub async fn save_connection(mut self) {
        self.icap_connection.mark_reader_finished();
        if self.icap_keepalive {
            self.icap_client.save_connection(self.icap_connection);
        }
    }
}
