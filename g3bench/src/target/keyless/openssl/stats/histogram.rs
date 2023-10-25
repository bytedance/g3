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

pub(crate) struct KeylessHistogram {
    total_time: SyncHistogram<u64>,
}

impl KeylessHistogram {
    pub(crate) fn new() -> (Self, KeylessHistogramRecorder) {
        let (h, r) = SyncHistogram::new(3).unwrap();
        (
            KeylessHistogram { total_time: h },
            KeylessHistogramRecorder { total_time: r },
        )
    }
}

impl BenchHistogram for KeylessHistogram {
    fn refresh(&mut self) {
        self.total_time.refresh(None).unwrap();
    }

    fn emit(&self, client: &StatsdClient) {
        macro_rules! emit_histogram {
            ($field:ident, $name:literal) => {
                let h = self.$field.inner();
                let min = h.min();
                client
                    .gauge_with_tags(concat!("keyless.", $name, ".min"), min)
                    .send();
                let max = h.max();
                client
                    .gauge_with_tags(concat!("keyless.", $name, ".max"), max)
                    .send();
                let mean = h.mean();
                client
                    .gauge_with_tags(concat!("keyless.", $name, ".mean"), mean)
                    .send();
                let pct50 = h.value_at_percentile(0.50);
                client
                    .gauge_with_tags(concat!("keyless.", $name, ".pct50"), pct50)
                    .send();
                let pct80 = h.value_at_percentile(0.80);
                client
                    .gauge_with_tags(concat!("keyless.", $name, ".pct80"), pct80)
                    .send();
                let pct90 = h.value_at_percentile(0.90);
                client
                    .gauge_with_tags(concat!("keyless.", $name, ".pct90"), pct90)
                    .send();
                let pct95 = h.value_at_percentile(0.95);
                client
                    .gauge_with_tags(concat!("keyless.", $name, ".pct95"), pct95)
                    .send();
                let pct98 = h.value_at_percentile(0.98);
                client
                    .gauge_with_tags(concat!("keyless.", $name, ".pct98"), pct98)
                    .send();
                let pct99 = h.value_at_percentile(0.99);
                client
                    .gauge_with_tags(concat!("keyless.", $name, ".pct99"), pct99)
                    .send();
            };
        }

        emit_histogram!(total_time, "time.total");
    }

    fn summary(&self) {
        Self::summary_histogram_title("# Duration Times");
        let total_time = self.total_time.inner();
        Self::summary_duration_line("Total:", total_time);
        Self::summary_newline();
        Self::summary_total_percentage(total_time);
    }
}

#[derive(Clone)]
pub(crate) struct KeylessHistogramRecorder {
    total_time: Recorder<u64>,
}

impl KeylessHistogramRecorder {
    pub(crate) fn record_total_time(&mut self, dur: Duration) {
        let _ = self.total_time.record(dur.as_nanos_u64());
    }
}
