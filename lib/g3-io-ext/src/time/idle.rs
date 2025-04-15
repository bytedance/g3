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
use std::time::Duration;

use tokio::time::{Instant, Interval};

pub struct IdleWheel {
    interval: Duration,
}

impl IdleWheel {
    pub fn spawn(interval: Duration) -> Arc<IdleWheel> {
        Arc::new(IdleWheel { interval })
    }

    pub fn register(&self) -> IdleInterval {
        IdleInterval {
            interval: tokio::time::interval_at(Instant::now() + self.interval, self.interval),
        }
    }
}

pub struct IdleInterval {
    interval: Interval,
}

impl IdleInterval {
    pub async fn tick(&mut self) -> usize {
        self.interval.tick().await;
        1
    }

    pub fn period(&self) -> Duration {
        self.interval.period()
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
