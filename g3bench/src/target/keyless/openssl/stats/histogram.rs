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

pub(crate) struct KeylessHistogram {
    total_time: KeepingHistogram<u64>,
}

impl KeylessHistogram {
    pub(crate) fn new() -> (Self, KeylessHistogramRecorder) {
        let (h, r) = KeepingHistogram::new();
        (
            KeylessHistogram { total_time: h },
            KeylessHistogramRecorder { total_time: r },
        )
    }
}

impl BenchHistogram for KeylessHistogram {
    fn refresh(&mut self) {
        self.total_time.refresh().unwrap();
    }

    fn emit(&self, client: &mut StatsdClient) {
        self.emit_histogram(client, self.total_time.inner(), "keyless.time.total");
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
    total_time: HistogramRecorder<u64>,
}

impl KeylessHistogramRecorder {
    pub(crate) fn record_total_time(&mut self, dur: Duration) {
        let _ = self.total_time.record(dur.as_nanos_u64());
    }
}
