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

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use tokio::sync::broadcast;

const CHANNEL_SIZE: usize = 8;

struct IdleWheelSlot {
    senders: [broadcast::Sender<()>; 16],
}

impl IdleWheelSlot {
    fn new() -> Self {
        IdleWheelSlot {
            senders: [
                broadcast::channel(CHANNEL_SIZE).0,
                broadcast::channel(CHANNEL_SIZE).0,
                broadcast::channel(CHANNEL_SIZE).0,
                broadcast::channel(CHANNEL_SIZE).0,
                broadcast::channel(CHANNEL_SIZE).0,
                broadcast::channel(CHANNEL_SIZE).0,
                broadcast::channel(CHANNEL_SIZE).0,
                broadcast::channel(CHANNEL_SIZE).0,
                broadcast::channel(CHANNEL_SIZE).0,
                broadcast::channel(CHANNEL_SIZE).0,
                broadcast::channel(CHANNEL_SIZE).0,
                broadcast::channel(CHANNEL_SIZE).0,
                broadcast::channel(CHANNEL_SIZE).0,
                broadcast::channel(CHANNEL_SIZE).0,
                broadcast::channel(CHANNEL_SIZE).0,
                broadcast::channel(CHANNEL_SIZE).0,
            ],
        }
    }

    fn subscribe(&self) -> broadcast::Receiver<()> {
        let mut id = 0;
        let mut min_count = self.senders[0].receiver_count();
        for i in 1..self.senders.len() {
            let count = self.senders[i].receiver_count();
            if min_count > count {
                id = i;
                min_count = count;
            }
        }
        // the selection of min sender is not atomic operation,
        // but we don't care as it doesn't need to be precise
        self.senders[id].subscribe()
    }

    fn has_receiver(&self) -> bool {
        self.senders
            .iter()
            .any(|sender| sender.receiver_count() > 0)
    }

    fn wake(&self) {
        self.senders.iter().for_each(|sender| {
            let _ = sender.send(());
        });
    }
}

pub struct IdleWheel {
    interval: Duration,
    slots: Vec<IdleWheelSlot>,
    register_index: AtomicUsize,
}

impl IdleWheel {
    pub fn spawn(interval: Duration) -> Arc<IdleWheel> {
        let mut interval_seconds = interval.as_secs() as usize;
        if interval.subsec_nanos() > 0 {
            interval_seconds += 1;
        }
        let slot_count = interval_seconds.max(2);
        let mut slots = Vec::with_capacity(slot_count);
        for _ in 0..slot_count {
            slots.push(IdleWheelSlot::new());
        }

        let register_index = AtomicUsize::new(slot_count - 1);
        let mut emit_index = 0;

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
                    let has_receiver = wheel_run.slots.iter().any(|slot| slot.has_receiver());
                    if !has_receiver {
                        break;
                    }
                }

                wheel_run.slots[emit_index].wake();
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
