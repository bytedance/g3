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
