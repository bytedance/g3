/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use hdrhistogram::{Counter, CreationError, Histogram, RecordError};
use tokio::sync::mpsc;

use crate::{HistogramRecorder, HistogramStats};

pub struct KeepingHistogram<T: Counter> {
    inner: Histogram<T>,
    receiver: mpsc::UnboundedReceiver<T>,
}

impl<T: Counter> KeepingHistogram<T> {
    pub fn new() -> (Self, HistogramRecorder<T>) {
        KeepingHistogram::with_sigfig(3).unwrap()
    }

    pub fn with_sigfig(sigfig: u8) -> Result<(Self, HistogramRecorder<T>), CreationError> {
        let inner = Histogram::new(sigfig)?;
        let (sender, receiver) = mpsc::unbounded_channel();
        Ok((
            KeepingHistogram { inner, receiver },
            HistogramRecorder::new(sender),
        ))
    }

    pub fn new_with_max(
        high: u64,
        sigfig: u8,
    ) -> Result<(Self, HistogramRecorder<T>), CreationError> {
        let inner = Histogram::new_with_max(high, sigfig)?;
        let (sender, receiver) = mpsc::unbounded_channel();
        Ok((
            KeepingHistogram { inner, receiver },
            HistogramRecorder::new(sender),
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
            KeepingHistogram { inner, receiver },
            HistogramRecorder::new(sender),
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

impl<T> KeepingHistogram<T>
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
                for v in &buf {
                    let _ = self.inner.record(v.as_u64());
                }
                buf.clear();
                stats.update(self.inner());
            }
        });
    }
}
