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
    index: AtomicUsize,
}

impl IdleWheel {
    pub fn spawn(interval: Duration) -> Arc<IdleWheel> {
        let interval_seconds = interval.as_secs().max(1) as usize;
        let mut slots = Vec::with_capacity(interval_seconds);
        for _ in 0..interval_seconds {
            let (sender, _) = broadcast::channel(CHANNEL_SIZE);
            slots.push(sender);
        }
        let wheel = Arc::new(IdleWheel {
            interval,
            slots,
            index: AtomicUsize::new(0),
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

                let mut index = wheel_run.index.load(Ordering::Acquire);
                if index >= wheel_run.slots.len() {
                    index = 0;
                } else {
                    index += 1;
                }

                // always fire the next value, so we can be sure that any IdleInterval.tick()
                // will be called after `interval_seconds` or `interval_seconds - 1` seconds
                let _ = wheel_run.slots[index].send(());
                wheel_run.index.store(index, Ordering::Release);
            }
        });

        wheel
    }

    pub fn get(&self) -> IdleInterval {
        let id = self.index.load(Ordering::Acquire);
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
    fn idle_duration(&self) -> IdleInterval;
    fn check_quit(&self, idle_count: usize) -> bool;
    fn check_force_quit(&self) -> Option<IdleForceQuitReason>;
}
