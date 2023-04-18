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

use std::sync::Arc;

use async_trait::async_trait;
use tokio::time::Instant;

use super::{BenchTaskContext, KeylessHistogramRecorder, KeylessOpensslArgs, KeylessRuntimeStats};

pub(super) struct KeylessOpensslTaskContext {
    args: Arc<KeylessOpensslArgs>,

    runtime_stats: Arc<KeylessRuntimeStats>,
    histogram_recorder: Option<KeylessHistogramRecorder>,
}

impl KeylessOpensslTaskContext {
    pub(super) fn new(
        args: &Arc<KeylessOpensslArgs>,
        runtime_stats: &Arc<KeylessRuntimeStats>,
        histogram_recorder: Option<KeylessHistogramRecorder>,
    ) -> anyhow::Result<Self> {
        Ok(KeylessOpensslTaskContext {
            args: Arc::clone(args),
            runtime_stats: Arc::clone(runtime_stats),
            histogram_recorder,
        })
    }
}

#[async_trait]
impl BenchTaskContext for KeylessOpensslTaskContext {
    fn mark_task_start(&self) {
        self.runtime_stats.add_task_total();
        self.runtime_stats.inc_task_alive();
    }

    fn mark_task_passed(&self) {
        self.runtime_stats.add_task_passed();
        self.runtime_stats.dec_task_alive();
    }

    fn mark_task_failed(&self) {
        self.runtime_stats.add_task_failed();
        self.runtime_stats.dec_task_alive();
    }

    async fn run(&mut self, task_id: usize, time_started: Instant) -> anyhow::Result<()> {
        let output = self.args.handle_action()?;
        let total_time = time_started.elapsed();
        if let Some(r) = &mut self.histogram_recorder {
            r.record_total_time(total_time);
        }
        self.args.dump_result(task_id, output);
        Ok(())
    }
}
