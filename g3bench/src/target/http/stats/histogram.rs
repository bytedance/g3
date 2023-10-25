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

use cadence::{Gauged, StatsdClient};

use g3_histogram::{Recorder, SyncHistogram};
use g3_types::ext::DurationExt;

use crate::target::BenchHistogram;

pub(crate) struct HttpHistogram {
    send_hdr_time: SyncHistogram<u64>,
    recv_hdr_time: SyncHistogram<u64>,
    total_time: SyncHistogram<u64>,
    conn_reuse_count: SyncHistogram<u64>,
}

impl HttpHistogram {
    pub(crate) fn new() -> (Self, HttpHistogramRecorder) {
        let (send_hdr_time_h, send_hdr_time_r) = SyncHistogram::new(3).unwrap();
        let (recv_hdr_time_h, recv_hdr_time_r) = SyncHistogram::new(3).unwrap();
        let (total_time_h, total_time_r) = SyncHistogram::new(3).unwrap();
        let (conn_reuse_count_h, conn_reuse_count_r) = SyncHistogram::new(3).unwrap();
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
        self.send_hdr_time.refresh(None).unwrap();
        self.recv_hdr_time.refresh(None).unwrap();
        self.total_time.refresh(None).unwrap();
        self.conn_reuse_count.refresh(None).unwrap();
    }

    fn emit(&self, client: &StatsdClient) {
        macro_rules! emit_histogram {
            ($field:ident, $name:literal) => {
                let h = self.$field.inner();
                let min = h.min();
                client
                    .gauge_with_tags(concat!("http.", $name, ".min"), min)
                    .send();
                let max = h.max();
                client
                    .gauge_with_tags(concat!("http.", $name, ".max"), max)
                    .send();
                let mean = h.mean();
                client
                    .gauge_with_tags(concat!("http.", $name, ".mean"), mean)
                    .send();
                let pct50 = h.value_at_percentile(0.50);
                client
                    .gauge_with_tags(concat!("http.", $name, ".pct50"), pct50)
                    .send();
                let pct80 = h.value_at_percentile(0.80);
                client
                    .gauge_with_tags(concat!("http.", $name, ".pct80"), pct80)
                    .send();
                let pct90 = h.value_at_percentile(0.90);
                client
                    .gauge_with_tags(concat!("http.", $name, ".pct90"), pct90)
                    .send();
                let pct95 = h.value_at_percentile(0.95);
                client
                    .gauge_with_tags(concat!("http.", $name, ".pct95"), pct95)
                    .send();
                let pct98 = h.value_at_percentile(0.98);
                client
                    .gauge_with_tags(concat!("http.", $name, ".pct98"), pct98)
                    .send();
                let pct99 = h.value_at_percentile(0.99);
                client
                    .gauge_with_tags(concat!("http.", $name, ".pct99"), pct99)
                    .send();
            };
        }

        emit_histogram!(send_hdr_time, "time.send_hdr");
        emit_histogram!(recv_hdr_time, "time.recv_hdr");
        emit_histogram!(total_time, "time.total");
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
    send_hdr_time: Recorder<u64>,
    recv_hdr_time: Recorder<u64>,
    total_time: Recorder<u64>,
    conn_reuse_count: Recorder<u64>,
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
