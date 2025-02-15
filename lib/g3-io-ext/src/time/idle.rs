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

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::broadcast;

const CHANNEL_SIZE: usize = 8;

pub struct IdleWheel {
    interval: Duration,
    slots: Vec<broadcast::Sender<()>>,
    register_index: AtomicUsize,
}

impl IdleWheel {
    pub fn spawn(interval: Duration) -> Arc<IdleWheel> {
        let interval_seconds = interval.as_secs().max(1) as usize;
        // always round to 1 more second, so the idle interval will be
        // `interval_seconds` - `interval_seconds + 1`
        let slot_count = interval_seconds + 1;
        let mut slots = Vec::with_capacity(slot_count);
        for _ in 0..slot_count {
            let (sender, _) = broadcast::channel(CHANNEL_SIZE);
            slots.push(sender);
        }

        let register_index = AtomicUsize::new(0);
        let mut emit_index = 1; // index 1 is valid as there are at least 2 slots

        let wheel = Arc::new(IdleWheel {
            interval,
            slots,
            register_index,
        });
        let wheel_run = wheel.clone();

        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(1));

            loop {
                interval.tick().await;
                if Arc::strong_count(&wheel_run) <= 1 {
                    let has_receiver = wheel_run.slots.iter().any(|v| v.receiver_count() > 0);
                    if !has_receiver {
                        break;
                    }
                }

                let _ = wheel_run.slots[emit_index].send(());
                // register new receivers to the last emit slot
                wheel_run
                    .register_index
                    .store(emit_index, Ordering::Release);
                emit_index += 1;
                if emit_index >= slot_count {
                    emit_index = 0;
                }
            }
        });

        wheel
    }

    pub fn register(&self) -> IdleInterval {
        let id = self.register_index.load(Ordering::Acquire);
        let receiver = self.slots[id].subscribe();
        IdleInterval {
            interval: self.interval,
            receiver,
        }
    }
}

pub struct IdleInterval {
    interval: Duration,
    receiver: broadcast::Receiver<()>,
}

impl IdleInterval {
    pub async fn tick(&mut self) -> usize {
        use broadcast::error::{RecvError, TryRecvError};

        match self.receiver.recv().await {
            Ok(_) => 1,
            Err(RecvError::Closed) => {
                // the sender won't be dropped if there are receivers
                unreachable!()
            }
            Err(RecvError::Lagged(mut n)) => {
                loop {
                    match self.receiver.try_recv() {
                        Ok(_) => n += 1,
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Lagged(m)) => n += m,
                        Err(TryRecvError::Closed) => unreachable!(),
                    }
                }
                usize::try_from(n).unwrap_or(CHANNEL_SIZE)
            }
        }
    }

    pub fn period(&self) -> Duration {
        self.interval
    }
}

#[derive(Clone, Copy, Debug)]
pub enum IdleForceQuitReason {
    UserBlocked,
    ServerQuit,
}

pub trait IdleCheck {
    fn interval_timer(&self) -> IdleInterval;
    fn check_quit(&self, idle_count: usize) -> bool;
    fn check_force_quit(&self) -> Option<IdleForceQuitReason>;
}
