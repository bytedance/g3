/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
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
