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
use hdrhistogram::{sync::Recorder, Histogram, SyncHistogram};

use g3_types::ext::DurationExt;

use crate::target::BenchHistogram;

pub(crate) struct KeylessHistogram {
    total_time: SyncHistogram<u64>,
    conn_reuse_count: SyncHistogram<u64>,
}

impl KeylessHistogram {
    pub(crate) fn new() -> Self {
        KeylessHistogram {
            total_time: Histogram::new(3).unwrap().into_sync(),
            conn_reuse_count: Histogram::new(3).unwrap().into_sync(),
        }
    }

    pub(crate) fn recorder(&self) -> KeylessHistogramRecorder {
        KeylessHistogramRecorder {
            total_time: self.total_time.recorder(),
            conn_reuse_count: self.conn_reuse_count.recorder(),
        }
    }
}

impl BenchHistogram for KeylessHistogram {
    fn refresh(&mut self) {
        self.total_time.refresh();
        self.conn_reuse_count.refresh();
    }

    fn emit(&self, client: &StatsdClient) {
        macro_rules! emit_histogram {
            ($field:ident, $name:literal) => {
                let min = self.$field.min();
                client
                    .gauge_with_tags(concat!("keyless.", $name, ".min"), min)
                    .send();
                let max = self.$field.max();
                client
                    .gauge_with_tags(concat!("keyless.", $name, ".max"), max)
                    .send();
                let mean = self.$field.mean();
                client
                    .gauge_with_tags(concat!("keyless.", $name, ".mean"), mean)
                    .send();
                let pct50 = self.$field.value_at_percentile(0.50);
                client
                    .gauge_with_tags(concat!("keyless.", $name, ".pct50"), pct50)
                    .send();
                let pct80 = self.$field.value_at_percentile(0.80);
                client
                    .gauge_with_tags(concat!("keyless.", $name, ".pct80"), pct80)
                    .send();
                let pct90 = self.$field.value_at_percentile(0.90);
                client
                    .gauge_with_tags(concat!("keyless.", $name, ".pct90"), pct90)
                    .send();
                let pct95 = self.$field.value_at_percentile(0.95);
                client
                    .gauge_with_tags(concat!("keyless.", $name, ".pct95"), pct95)
                    .send();
                let pct98 = self.$field.value_at_percentile(0.98);
                client
                    .gauge_with_tags(concat!("keyless.", $name, ".pct98"), pct98)
                    .send();
                let pct99 = self.$field.value_at_percentile(0.99);
                client
                    .gauge_with_tags(concat!("keyless.", $name, ".pct99"), pct99)
                    .send();
            };
        }

        emit_histogram!(total_time, "time.total");
    }

    fn summary(&self) {
        Self::summary_histogram_title("# Connection Re-Usage:");
        Self::summary_data_line("Req/Conn:", &self.conn_reuse_count);
        Self::summary_histogram_title("# Duration Times");
        Self::summary_duration_line("Total:", &self.total_time);
        Self::summary_newline();
        Self::summary_total_percentage(&self.total_time);
    }
}

pub(crate) struct KeylessHistogramRecorder {
    total_time: Recorder<u64>,
    conn_reuse_count: Recorder<u64>,
}

impl KeylessHistogramRecorder {
    pub(crate) fn record_total_time(&mut self, dur: Duration) {
        let _ = self.total_time.record(dur.as_nanos_u64());
    }

    pub(crate) fn record_conn_reuse_count(&mut self, count: u64) {
        let _ = self.conn_reuse_count.record(count);
    }
}
