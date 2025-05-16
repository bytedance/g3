/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use hdrhistogram::Counter;
use tokio::sync::mpsc;

#[derive(Clone)]
pub struct HistogramRecorder<T: Counter> {
    sender: mpsc::UnboundedSender<T>,
}

impl<T: Counter> HistogramRecorder<T> {
    pub(crate) fn new(sender: mpsc::UnboundedSender<T>) -> Self {
        HistogramRecorder { sender }
    }

    pub fn record(&self, v: T) -> Result<(), mpsc::error::SendError<T>> {
        self.sender.send(v)
    }
}
