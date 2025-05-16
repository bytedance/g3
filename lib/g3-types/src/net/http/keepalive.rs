/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::time::Duration;

const DEFAULT_HTTP_KEEPALIVE_IDLE: u64 = 60;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HttpKeepAliveConfig {
    enabled: bool,
    idle_expire: Duration,
}

impl Default for HttpKeepAliveConfig {
    fn default() -> Self {
        HttpKeepAliveConfig {
            enabled: true,
            idle_expire: Duration::from_secs(DEFAULT_HTTP_KEEPALIVE_IDLE),
        }
    }
}

impl HttpKeepAliveConfig {
    pub fn new(idle_expire: Duration) -> Self {
        HttpKeepAliveConfig {
            enabled: true,
            idle_expire,
        }
    }

    pub fn set_enable(&mut self, enable: bool) {
        self.enabled = enable;
    }

    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_idle_expire(&mut self, idle_expire: Duration) {
        self.idle_expire = idle_expire;
    }

    #[inline]
    pub fn idle_expire(&self) -> Duration {
        if self.enabled {
            self.idle_expire
        } else {
            Duration::ZERO
        }
    }

    #[must_use]
    pub fn adjust_to(self, other: Self) -> Self {
        let idle_expire = self.idle_expire.min(other.idle_expire);
        let enabled = self.enabled && other.enabled; // only if both enabled
        HttpKeepAliveConfig {
            enabled,
            idle_expire,
        }
    }
}
