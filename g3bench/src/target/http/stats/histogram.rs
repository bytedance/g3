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

use std::time::Duration;

use g3_histogram::{HistogramRecorder, KeepingHistogram};
use g3_statsd_client::StatsdClient;
use g3_types::ext::DurationExt;

use crate::target::BenchHistogram;

pub(crate) struct HttpHistogram {
    send_hdr_time: KeepingHistogram<u64>,
    recv_hdr_time: KeepingHistogram<u64>,
    total_time: KeepingHistogram<u64>,
    conn_reuse_count: KeepingHistogram<u64>,
}

impl HttpHistogram {
    pub(crate) fn new() -> (Self, HttpHistogramRecorder) {
        let (send_hdr_time_h, send_hdr_time_r) = KeepingHistogram::new();
        let (recv_hdr_time_h, recv_hdr_time_r) = KeepingHistogram::new();
        let (total_time_h, total_time_r) = KeepingHistogram::new();
        let (conn_reuse_count_h, conn_reuse_count_r) = KeepingHistogram::new();
        let h = HttpHistogram {
            send_hdr_time: send_hdr_time_h,
            recv_hdr_time: recv_hdr_time_h,
            total_time: total_time_h,
            conn_reuse_count: conn_reuse_count_h,
        };
        let r = HttpHistogramRecorder {
            send_hdr_time: send_hdr_time_r,
            recv_hdr_time: recv_hdr_time_r,
            total_time: total_time_r,
            conn_reuse_count: conn_reuse_count_r,
        };
        (h, r)
    }
}

impl BenchHistogram for HttpHistogram {
    fn refresh(&mut self) {
        self.send_hdr_time.refresh().unwrap();
        self.recv_hdr_time.refresh().unwrap();
        self.total_time.refresh().unwrap();
        self.conn_reuse_count.refresh().unwrap();
    }

    fn emit(&self, client: &mut StatsdClient) {
        self.emit_histogram(client, self.send_hdr_time.inner(), "http.time.send_hdr");
        self.emit_histogram(client, self.recv_hdr_time.inner(), "http.time.recv_hdr");
        self.emit_histogram(client, self.total_time.inner(), "http.time.total");
    }

    fn summary(&self) {
        Self::summary_histogram_title("# Connection Re-Usage:");
        Self::summary_data_line("Req/Conn:", self.conn_reuse_count.inner());
        Self::summary_histogram_title("# Duration Times");
        Self::summary_duration_line("SendHdr:", self.send_hdr_time.inner());
        Self::summary_duration_line("RecvHdr:", self.recv_hdr_time.inner());
        Self::summary_duration_line("Total:", self.total_time.inner());
        Self::summary_newline();
        Self::summary_total_percentage(self.total_time.inner());
    }
}

#[derive(Clone)]
pub(crate) struct HttpHistogramRecorder {
    send_hdr_time: HistogramRecorder<u64>,
    recv_hdr_time: HistogramRecorder<u64>,
    total_time: HistogramRecorder<u64>,
    conn_reuse_count: HistogramRecorder<u64>,
}

impl HttpHistogramRecorder {
    pub(crate) fn record_send_hdr_time(&mut self, dur: Duration) {
        let _ = self.send_hdr_time.record(dur.as_nanos_u64());
    }

    pub(crate) fn record_recv_hdr_time(&mut self, dur: Duration) {
        let _ = self.recv_hdr_time.record(dur.as_nanos_u64());
    }

    pub(crate) fn record_total_time(&mut self, dur: Duration) {
        let _ = self.total_time.record(dur.as_nanos_u64());
    }

    pub(crate) fn record_conn_reuse_count(&mut self, count: u64) {
        let _ = self.conn_reuse_count.record(count);
    }
}
