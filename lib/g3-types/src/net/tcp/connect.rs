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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TcpConnectConfig {
    max_tries: usize,
    each_timeout: Duration,
}

impl Default for TcpConnectConfig {
    fn default() -> Self {
        TcpConnectConfig {
            max_tries: 3,
            each_timeout: Duration::from_secs(30),
        }
    }
}

impl TcpConnectConfig {
    pub fn set_max_retry(&mut self, max_retry: usize) {
        self.max_tries = max_retry + 1;
    }

    #[inline]
    pub fn max_tries(&self) -> usize {
        self.max_tries
    }

    pub fn set_each_timeout(&mut self, each_timeout: Duration) {
        self.each_timeout = each_timeout;
    }

    #[inline]
    pub fn each_timeout(&self) -> Duration {
        self.each_timeout
    }

    pub fn limit_to(&mut self, other: &Self) {
        self.max_tries = self.max_tries.min(other.max_tries);
        self.each_timeout = self.each_timeout.min(other.each_timeout);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HappyEyeballsConfig {
    first_resolution_delay: Duration,
    second_resolution_timeout: Duration,
    first_address_family_count: usize,
    connection_attempt_delay: Duration,
}

impl Default for HappyEyeballsConfig {
    fn default() -> Self {
        HappyEyeballsConfig {
            first_resolution_delay: Duration::from_millis(50),
            second_resolution_timeout: Duration::from_secs(2),
            first_address_family_count: 1,
            connection_attempt_delay: Duration::from_millis(250),
        }
    }
}

impl HappyEyeballsConfig {
    #[inline]
    pub fn resolution_delay(&self) -> Duration {
        self.first_resolution_delay
    }

    pub fn set_resolution_delay(&mut self, delay: Duration) {
        self.first_resolution_delay = delay;
    }

    #[inline]
    pub fn second_resolution_timeout(&self) -> Duration {
        self.second_resolution_timeout
    }

    pub fn set_second_resolution_timeout(&mut self, timeout: Duration) {
        self.second_resolution_timeout = timeout;
    }

    #[inline]
    pub fn first_address_family_count(&self) -> usize {
        self.first_address_family_count
    }

    pub fn set_first_address_family_count(&mut self, count: usize) {
        self.first_address_family_count = count;
    }

    #[inline]
    pub fn connection_attempt_delay(&self) -> Duration {
        self.connection_attempt_delay
    }

    pub fn set_connection_attempt_delay(&mut self, delay: Duration) {
        self.connection_attempt_delay =
            delay.clamp(Duration::from_millis(100), Duration::from_secs(2))
    }

    pub fn merge_list<T>(&self, tried: usize, ips: &mut Vec<T>, new: Vec<T>) {
        let mut id = self.first_address_family_count.saturating_sub(tried);
        for ip in new {
            if id < ips.len() {
                ips.insert(id, ip);
                id += 2;
            } else {
                ips.push(ip);
                id += 1;
            }
        }
    }
}
