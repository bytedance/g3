/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImapInterceptionConfig {
    pub greeting_timeout: Duration,
    pub authenticate_timeout: Duration,
    pub logout_wait_timeout: Duration,
    pub command_line_max_size: usize,
    pub response_line_max_size: usize,
    pub forward_max_idle_count: usize,
    pub transfer_max_idle_count: usize,
}

impl Default for ImapInterceptionConfig {
    fn default() -> Self {
        ImapInterceptionConfig {
            greeting_timeout: Duration::from_secs(300),
            authenticate_timeout: Duration::from_secs(300),
            logout_wait_timeout: Duration::from_secs(10),
            command_line_max_size: 4096,
            response_line_max_size: 4096,
            forward_max_idle_count: 30,
            transfer_max_idle_count: 5,
        }
    }
}
