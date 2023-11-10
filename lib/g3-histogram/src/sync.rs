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

use hdrhistogram::{Counter, CreationError, Histogram, RecordError};
use tokio::sync::mpsc;

use crate::HistogramStats;

pub struct DurationHistogram<T: Counter> {
    inner: Histogram<T>,
    receiver: mpsc::UnboundedReceiver<T>,
}

#[derive(Clone)]
pub struct HistogramRecorder<T: Counter> {
    sender: mpsc::UnboundedSender<T>,
}

impl<T: Counter> DurationHistogram<T> {
    pub fn new() -> (Self, HistogramRecorder<T>) {
        DurationHistogram::with_sigfig(3).unwrap()
    }

    pub fn with_sigfig(sigfig: u8) -> Result<(Self, HistogramRecorder<T>), CreationError> {
        let inner = Histogram::new(sigfig)?;
        let (sender, receiver) = mpsc::unbounded_channel();
        Ok((
            DurationHistogram { inner, receiver },
            HistogramRecorder { sender },
        ))
    }

    pub fn new_with_max(
        high: u64,
        sigfig: u8,
    ) -> Result<(Self, HistogramRecorder<T>), CreationError> {
        let inner = Histogram::new_with_max(high, sigfig)?;
        let (sender, receiver) = mpsc::unbounded_channel();
        Ok((
            DurationHistogram { inner, receiver },
            HistogramRecorder { sender },
        ))
    }

    pub fn new_with_bounds(
        low: u64,
        high: u64,
        sigfig: u8,
    ) -> Result<(Self, HistogramRecorder<T>), CreationError> {
        let inner = Histogram::new_with_bounds(low, high, sigfig)?;
        let (sender, receiver) = mpsc::unbounded_channel();
        Ok((
            DurationHistogram { inner, receiver },
            HistogramRecorder { sender },
        ))
    }

    pub fn auto(&mut self, enabled: bool) {
        self.inner.auto(enabled);
    }

    pub fn refresh(&mut self) -> Result<(), RecordError> {
        use mpsc::error::TryRecvError;

        loop {
            match self.receiver.try_recv() {
                Ok(v) => self.inner.record(v.as_u64())?,
                Err(TryRecvError::Empty) => return Ok(()),
                Err(TryRecvError::Disconnected) => return Ok(()),
            }
        }
    }

    pub fn inner(&self) -> &Histogram<T> {
        &self.inner
    }
}

impl<T> DurationHistogram<T>
where
    T: Counter + Send + 'static,
{
    pub fn spawn_refresh(mut self, stats: Arc<HistogramStats>) {
        tokio::spawn(async move {
            const BATCH_SIZE: usize = 16;

            let mut buf = Vec::with_capacity(BATCH_SIZE);
            loop {
                let count = self.receiver.recv_many(&mut buf, BATCH_SIZE).await;
                if count == 0 {
                    break;
                }
                for v in buf.iter().take(count) {
                    let _ = self.inner.record(v.as_u64());
                }
                stats.update(self.inner());
            }
        });
    }
}

impl<T: Counter> HistogramRecorder<T> {
    pub fn record(&self, v: T) -> Result<(), mpsc::error::SendError<T>> {
        self.sender.send(v)
    }
}
