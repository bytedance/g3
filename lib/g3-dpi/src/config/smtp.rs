/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SmtpInterceptionConfig {
    pub greeting_timeout: Duration,
    pub quit_wait_timeout: Duration,
    pub command_wait_timeout: Duration,
    pub response_wait_timeout: Duration,
    pub data_initiation_timeout: Duration,
    pub data_termination_timeout: Duration,
    pub allow_on_demand_mail_relay: bool,
    pub allow_data_chunking: bool,
    pub allow_burl_data: bool,
}

impl Default for SmtpInterceptionConfig {
    fn default() -> Self {
        SmtpInterceptionConfig {
            greeting_timeout: Duration::from_secs(300),
            quit_wait_timeout: Duration::from_secs(60),
            command_wait_timeout: Duration::from_secs(300),
            response_wait_timeout: Duration::from_secs(300),
            data_initiation_timeout: Duration::from_secs(120),
            data_termination_timeout: Duration::from_secs(600),
            allow_on_demand_mail_relay: false,
            allow_data_chunking: false,
            allow_burl_data: false,
        }
    }
}
