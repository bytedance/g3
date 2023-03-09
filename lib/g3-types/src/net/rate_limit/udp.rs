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

use std::cmp::Ordering;

use anyhow::anyhow;

use super::{get_nonzero_smaller, RATE_LIMIT_SHIFT_MILLIS_MAX};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct UdpSockSpeedLimitConfig {
    pub shift_millis: u8,
    pub max_north_packets: usize, // upload
    pub max_south_packets: usize, // download
    pub max_north_bytes: usize,   // upload
    pub max_south_bytes: usize,   // download
}

impl UdpSockSpeedLimitConfig {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.shift_millis > 0 && self.shift_millis > RATE_LIMIT_SHIFT_MILLIS_MAX {
            return Err(anyhow!(
                "the shift value should be less than {RATE_LIMIT_SHIFT_MILLIS_MAX}",
            ));
        }
        Ok(())
    }

    #[must_use]
    pub fn shrink_as_smaller(&self, other: &Self) -> Self {
        if self.shift_millis == 0 {
            return *other;
        }
        if other.shift_millis == 0 {
            return *self;
        }

        let shift_millis = self.shift_millis;
        let (other_north_packets, other_north_bytes, other_south_packets, other_south_bytes) =
            match shift_millis.cmp(&other.shift_millis) {
                Ordering::Equal => (
                    other.max_north_packets,
                    other.max_north_bytes,
                    other.max_south_packets,
                    other.max_south_bytes,
                ),
                Ordering::Less => {
                    let shift = other.shift_millis - shift_millis;
                    (
                        other.max_north_packets >> shift,
                        other.max_north_bytes >> shift,
                        other.max_south_packets >> shift,
                        other.max_south_bytes >> shift,
                    )
                }
                Ordering::Greater => {
                    let shift = shift_millis - other.shift_millis;
                    (
                        other.max_north_packets << shift,
                        other.max_north_bytes << shift,
                        other.max_south_packets << shift,
                        other.max_south_bytes << shift,
                    )
                }
            };

        UdpSockSpeedLimitConfig {
            shift_millis,
            max_north_packets: get_nonzero_smaller(self.max_north_packets, other_north_packets),
            max_north_bytes: get_nonzero_smaller(self.max_north_bytes, other_north_bytes),
            max_south_packets: get_nonzero_smaller(self.max_south_packets, other_south_packets),
            max_south_bytes: get_nonzero_smaller(self.max_south_bytes, other_south_bytes),
        }
    }
}
