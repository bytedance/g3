/*
 * Copyright 2025 ByteDance and/or its affiliates.
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

use tokio::sync::mpsc;

use super::{AggregateStore, Command};

const BATCH_SIZE: usize = 128;

pub(super) struct WorkerStore {
    inner: AggregateStore,
    receiver: mpsc::Receiver<Command>,
    global_sender: mpsc::Sender<Command>,
}

impl WorkerStore {
    pub(super) fn new(
        receiver: mpsc::Receiver<Command>,
        global_sender: mpsc::Sender<Command>,
    ) -> Self {
        WorkerStore {
            inner: AggregateStore::default(),
            receiver,
            global_sender,
        }
    }

    pub(super) async fn into_running(mut self) {
        let mut buffer = Vec::with_capacity(BATCH_SIZE);
        loop {
            let nr = self.receiver.recv_many(&mut buffer, BATCH_SIZE).await;
            if nr == 0 {
                break;
            }

            while let Some(cmd) = buffer.pop() {
                match cmd {
                    Command::Add(record) => self.inner.add(record),
                    Command::Emit(sender) => match self.inner.emit(&self.global_sender).await {
                        Ok(n) => {
                            let _ = sender.send(n);
                        }
                        Err(n) => {
                            let _ = sender.send(n);
                        }
                    },
                }
            }
        }
    }
}
