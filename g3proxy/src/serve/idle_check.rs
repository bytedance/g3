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
use std::time::Duration;

use g3_io_ext::{IdleCheck, IdleForceQuitReason};

use super::ServerQuitPolicy;
use crate::auth::User;

pub(crate) struct ServerIdleChecker {
    pub(crate) idle_duration: Duration,
    pub(crate) user: Option<Arc<User>>,
    pub(crate) task_max_idle_count: i32,
    pub(crate) server_quit_policy: Arc<ServerQuitPolicy>,
}

impl IdleCheck for ServerIdleChecker {
    fn idle_duration(&self) -> Duration {
        self.idle_duration
    }

    fn check_quit(&self, idle_count: i32) -> bool {
        if let Some(user) = &self.user {
            idle_count > user.task_max_idle_count()
        } else {
            idle_count > self.task_max_idle_count
        }
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
