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

use crate::ext::OptionExt;

const DEFAULT_TCP_KEEPALIVE_IDLE: Duration = Duration::from_secs(60);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TcpKeepAliveConfig {
    enabled: bool,
    idle_time: Duration,
    probe_interval: Option<Duration>,
    probe_count: Option<u32>,
}

impl Default for TcpKeepAliveConfig {
    fn default() -> Self {
        TcpKeepAliveConfig {
            enabled: false,
            idle_time: DEFAULT_TCP_KEEPALIVE_IDLE,
            probe_interval: None,
            probe_count: None,
        }
    }
}

impl TcpKeepAliveConfig {
    pub fn default_enabled() -> Self {
        TcpKeepAliveConfig {
            enabled: true,
            idle_time: DEFAULT_TCP_KEEPALIVE_IDLE,
            probe_interval: None,
            probe_count: None,
        }
    }

    pub fn set_enable(&mut self, enable: bool) {
        self.enabled = enable;
    }

    #[inline]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_idle_time(&mut self, idle_time: Duration) {
        self.idle_time = idle_time;
    }

    #[inline]
    pub fn idle_time(&self) -> Duration {
        self.idle_time
    }

    pub fn set_probe_interval(&mut self, probe_interval: Duration) {
        self.probe_interval = Some(probe_interval);
    }

    #[inline]
    pub fn probe_interval(&self) -> Option<Duration> {
        self.probe_interval
    }

    pub fn set_probe_count(&mut self, probe_count: u32) {
        self.probe_count = Some(probe_count);
    }

    #[inline]
    pub fn probe_count(&self) -> Option<u32> {
        self.probe_count
    }

    #[must_use]
    pub fn adjust_to(self, other: Self) -> Self {
        if self.enabled || other.enabled {
            let idle_time = self.idle_time.min(other.idle_time);
            let probe_interval = self.probe_interval.existed_min(other.probe_interval);
            let probe_count = self.probe_count.existed_min(other.probe_count);

            TcpKeepAliveConfig {
                enabled: true,
                idle_time,
                probe_interval,
                probe_count,
            }
        } else {
            TcpKeepAliveConfig::default()
        }
    }
}
