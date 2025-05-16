/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::atomic::{AtomicBool, Ordering};

pub struct ServerQuitPolicy {
    force_quit: AtomicBool,
    force_quit_scheduled: AtomicBool,
}

impl Default for ServerQuitPolicy {
    fn default() -> Self {
        ServerQuitPolicy {
            force_quit: AtomicBool::new(false),
            force_quit_scheduled: AtomicBool::new(false),
        }
    }
}

impl ServerQuitPolicy {
    pub fn force_quit(&self) -> bool {
        self.force_quit.load(Ordering::Relaxed)
    }

    pub fn set_force_quit(&self) {
        self.force_quit.store(true, Ordering::Relaxed);
    }

    pub fn force_quit_scheduled(&self) -> bool {
        self.force_quit_scheduled.load(Ordering::Relaxed)
    }

    pub fn set_force_quit_scheduled(&self) {
        self.force_quit_scheduled.store(true, Ordering::Relaxed);
    }
}
