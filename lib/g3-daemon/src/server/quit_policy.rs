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
