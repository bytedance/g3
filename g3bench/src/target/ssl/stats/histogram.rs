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

pub(crate) struct SslHistogram {
    total_time: SyncHistogram<u64>,
}

impl SslHistogram {
    pub(crate) fn new() -> Self {
        SslHistogram {
            total_time: Histogram::new(3).unwrap().into_sync(),
        }
    }

    pub(crate) fn recorder(&self) -> SslHistogramRecorder {
        SslHistogramRecorder {
            total_time: self.total_time.recorder(),
        }
    }
}

impl BenchHistogram for SslHistogram {
    fn refresh(&mut self) {
        self.total_time.refresh();
    }

    fn emit(&self, client: &StatsdClient) {
        macro_rules! emit_histogram {
            ($field:ident, $name:literal) => {
                let min = self.$field.min();
                client
                    .gauge_with_tags(concat!("ssl.", $name, ".min"), min)
                    .send();
                let max = self.$field.max();
                client
                    .gauge_with_tags(concat!("ssl.", $name, ".max"), max)
                    .send();
                let mean = self.$field.mean();
                client
                    .gauge_with_tags(concat!("ssl.", $name, ".mean"), mean)
                    .send();
                let pct50 = self.$field.value_at_percentile(0.50);
                client
                    .gauge_with_tags(concat!("ssl.", $name, ".pct50"), pct50)
                    .send();
                let pct80 = self.$field.value_at_percentile(0.80);
                client
                    .gauge_with_tags(concat!("ssl.", $name, ".pct80"), pct80)
                    .send();
                let pct90 = self.$field.value_at_percentile(0.90);
                client
                    .gauge_with_tags(concat!("ssl.", $name, ".pct90"), pct90)
                    .send();
                let pct95 = self.$field.value_at_percentile(0.95);
                client
                    .gauge_with_tags(concat!("ssl.", $name, ".pct95"), pct95)
                    .send();
                let pct98 = self.$field.value_at_percentile(0.98);
                client
                    .gauge_with_tags(concat!("ssl.", $name, ".pct98"), pct98)
                    .send();
                let pct99 = self.$field.value_at_percentile(0.99);
                client
                    .gauge_with_tags(concat!("ssl.", $name, ".pct99"), pct99)
                    .send();
            };
        }

        emit_histogram!(total_time, "time.total");
    }

    fn summary(&self) {
        Self::summary_histogram_title("# Duration Times");
        Self::summary_duration_line("Total:", &self.total_time);
        Self::summary_newline();
        Self::summary_total_percentage(&self.total_time);
    }
}

pub(crate) struct SslHistogramRecorder {
    total_time: Recorder<u64>,
}

impl SslHistogramRecorder {
    pub(crate) fn record_total_time(&mut self, dur: Duration) {
        let _ = self.total_time.record(dur.as_nanos_u64());
    }
}
