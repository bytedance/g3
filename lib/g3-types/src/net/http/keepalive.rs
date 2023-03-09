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
