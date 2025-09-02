/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::time::Duration;

use g3_histogram::{HistogramRecorder, KeepingHistogram};
use g3_statsd_client::StatsdClient;
use g3_std_ext::time::DurationExt;

use crate::target::BenchHistogram;

pub(crate) struct WebsocketHistogram {
    total_time: KeepingHistogram<u64>,
    conn_reuse_count: KeepingHistogram<u64>,
}

impl WebsocketHistogram {
    pub(crate) fn new() -> (Self, WebsocketHistogramRecorder) {
        let (total_time_h, total_time_r) = KeepingHistogram::new();
        let (conn_reuse_count_h, conn_reuse_count_r) = KeepingHistogram::new();
        let h = WebsocketHistogram {
            total_time: total_time_h,
            conn_reuse_count: conn_reuse_count_h,
        };
        let r = WebsocketHistogramRecorder {
            total_time: total_time_r,
            conn_reuse_count: conn_reuse_count_r,
        };
        (h, r)
    }
}

impl BenchHistogram for WebsocketHistogram {
    fn refresh(&mut self) {
        self.total_time.refresh().unwrap();
        self.conn_reuse_count.refresh().unwrap();
    }

    fn emit(&self, client: &mut StatsdClient) {
        self.emit_histogram(client, self.total_time.inner(), "http.time.total");
    }

    fn summary(&self) {
        Self::summary_histogram_title("# Connection Re-Usage:");
        Self::summary_data_line("Req/Conn:", self.conn_reuse_count.inner());
        Self::summary_histogram_title("# Duration Times");
        Self::summary_duration_line("Total:", self.total_time.inner());
        Self::summary_newline();
        Self::summary_total_percentage(self.total_time.inner());
    }
}

#[derive(Clone)]
pub(crate) struct WebsocketHistogramRecorder {
    total_time: HistogramRecorder<u64>,
    conn_reuse_count: HistogramRecorder<u64>,
}

impl WebsocketHistogramRecorder {
    pub(crate) fn record_total_time(&mut self, dur: Duration) {
        let _ = self.total_time.record(dur.as_nanos_u64());
    }

    pub(crate) fn record_conn_reuse_count(&mut self, count: u64) {
        let _ = self.conn_reuse_count.record(count);
    }
}
