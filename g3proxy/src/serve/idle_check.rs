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

use std::sync::Arc;

use g3_io_ext::{IdleCheck, IdleForceQuitReason, IdleInterval, IdleWheel};

use super::ServerQuitPolicy;
use crate::auth::User;

pub(crate) struct ServerIdleChecker {
    pub(crate) idle_wheel: Arc<IdleWheel>,
    pub(crate) user: Option<Arc<User>>,
    pub(crate) max_idle_count: usize,
    pub(crate) server_quit_policy: Arc<ServerQuitPolicy>,
}

impl ServerIdleChecker {
    pub(crate) fn new(
        idle_wheel: Arc<IdleWheel>,
        user: Option<Arc<User>>,
        task_max_idle_count: usize,
        server_quit_policy: Arc<ServerQuitPolicy>,
    ) -> Self {
        let max_idle_count = user
            .as_ref()
            .and_then(|u| u.task_max_idle_count())
            .unwrap_or(task_max_idle_count);
        ServerIdleChecker {
            idle_wheel,
            user,
            max_idle_count,
            server_quit_policy,
        }
    }
}

impl IdleCheck for ServerIdleChecker {
    fn interval_timer(&self) -> IdleInterval {
        self.idle_wheel.register()
    }

    fn check_quit(&self, idle_count: usize) -> bool {
        idle_count > self.max_idle_count
    }

    fn check_force_quit(&self) -> Option<IdleForceQuitReason> {
        if let Some(user) = &self.user {
            if user.is_blocked() {
                return Some(IdleForceQuitReason::UserBlocked);
            }
        }

        if self.server_quit_policy.force_quit() {
            return Some(IdleForceQuitReason::ServerQuit);
        }

        None
    }
}
