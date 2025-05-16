/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::time::Duration;

#[cfg(feature = "yaml")]
mod yaml;

const MAXIMUM_LIST_ALL_TIMEOUT: Duration = Duration::from_secs(300);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FtpClientConfig {
    pub control: FtpControlConfig,
    pub transfer: FtpTransferConfig,
    pub connect_timeout: Duration,
    pub greeting_timeout: Duration,
    pub always_try_epsv: bool,
}

impl Default for FtpClientConfig {
    fn default() -> Self {
        FtpClientConfig {
            control: FtpControlConfig::default(),
            transfer: FtpTransferConfig::default(),
            connect_timeout: Duration::from_secs(30),
            greeting_timeout: Duration::from_secs(10),
            always_try_epsv: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FtpControlConfig {
    pub max_line_len: usize,
    pub max_multi_lines: usize,
    pub command_timeout: Duration,
}

impl Default for FtpControlConfig {
    fn default() -> Self {
        FtpControlConfig {
            max_line_len: 2048,
            max_multi_lines: 128,
            command_timeout: Duration::from_secs(10),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FtpTransferConfig {
    pub end_wait_timeout: Duration,
    pub list_max_entries: usize,
    pub list_max_line_len: usize,
    pub(crate) list_all_timeout: Duration,
}

impl Default for FtpTransferConfig {
    fn default() -> Self {
        FtpTransferConfig {
            end_wait_timeout: Duration::from_secs(2),
            list_max_entries: 1024,
            list_max_line_len: 2048,
            list_all_timeout: Duration::from_secs(120),
        }
    }
}

impl FtpTransferConfig {
    pub fn set_list_all_timeout(&mut self, timeout: Duration) {
        self.list_all_timeout = timeout.min(MAXIMUM_LIST_ALL_TIMEOUT);
    }
}
