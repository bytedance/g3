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
use std::time::Duration;

use hdrhistogram::{Counter, CreationError, Histogram};
use tokio::runtime::Handle;
use tokio::sync::mpsc;

use crate::{HistogramRecorder, HistogramStats};

pub struct RotatingHistogram<T: Counter> {
    rotate_interval: Duration,
    inner: Histogram<T>,
    receiver: mpsc::UnboundedReceiver<T>,
}

impl<T: Counter> RotatingHistogram<T> {
    pub fn new(rotate_interval: Duration) -> (Self, HistogramRecorder<T>) {
        RotatingHistogram::with_sigfig(rotate_interval, 3).unwrap()
    }

    pub fn with_sigfig(
        rotate_interval: Duration,
        sigfig: u8,
    ) -> Result<(Self, HistogramRecorder<T>), CreationError> {
        let inner = Histogram::new(sigfig)?;
        let (sender, receiver) = mpsc::unbounded_channel();
        Ok((
            RotatingHistogram {
                rotate_interval,
                inner,
                receiver,
            },
            HistogramRecorder::new(sender),
        ))
    }

    pub fn new_with_max(
        rotate_interval: Duration,
        high: u64,
        sigfig: u8,
    ) -> Result<(Self, HistogramRecorder<T>), CreationError> {
        let inner = Histogram::new_with_max(high, sigfig)?;
        let (sender, receiver) = mpsc::unbounded_channel();
        Ok((
            RotatingHistogram {
                rotate_interval,
                inner,
                receiver,
            },
            HistogramRecorder::new(sender),
        ))
    }

    pub fn new_with_bounds(
        rotate_interval: Duration,
        low: u64,
        high: u64,
        sigfig: u8,
    ) -> Result<(Self, HistogramRecorder<T>), CreationError> {
        let inner = Histogram::new_with_bounds(low, high, sigfig)?;
        let (sender, receiver) = mpsc::unbounded_channel();
        Ok((
            RotatingHistogram {
                rotate_interval,
                inner,
                receiver,
            },
            HistogramRecorder::new(sender),
        ))
    }

    pub fn auto(&mut self, enabled: bool) {
        self.inner.auto(enabled);
    }
}

impl<T> RotatingHistogram<T>
where
    T: Counter + Send + 'static,
{
    pub fn spawn_refresh(mut self, stats: Arc<HistogramStats>, handle: Option<Handle>) {
        let handle = handle.unwrap_or_else(Handle::current);
        handle.spawn(async move {
            const BATCH_SIZE: usize = 16;
            let mut buf = Vec::with_capacity(BATCH_SIZE);
            let mut rotate_interval = tokio::time::interval(self.rotate_interval);

            loop {
                tokio::select! {
                    biased;

                    n = self.receiver.recv_many(&mut buf, BATCH_SIZE) => {
                        if n == 0 {
                            break;
                        }
                        for v in buf.iter().take(n) {
                            let _ = self.inner.record(v.as_u64());
                        }
                        buf.clear();
                    }
                    _ = rotate_interval.tick() => {
                        if !self.inner.is_empty() {
                            stats.update(&self.inner);
                            self.inner.reset();
                        }
                    }
                }
            }
        });
    }
}
