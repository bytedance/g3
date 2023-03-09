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
pub struct TcpSockSpeedLimitConfig {
    pub shift_millis: u8,
    pub max_north: usize, // upload
    pub max_south: usize, // download
}

impl TcpSockSpeedLimitConfig {
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.shift_millis > 0 {
            if self.shift_millis > RATE_LIMIT_SHIFT_MILLIS_MAX {
                return Err(anyhow!(
                    "the shift value should be less than {RATE_LIMIT_SHIFT_MILLIS_MAX}",
                ));
            }
            if self.max_north == 0 {
                return Err(anyhow!(
                    "the upload limit should not be 0 as this limit is enabled"
                ));
            }
            if self.max_south == 0 {
                return Err(anyhow!(
                    "the download limit should not be 0 as this limit is enabled"
                ));
            }
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
        let (other_north, other_south) = match shift_millis.cmp(&other.shift_millis) {
            Ordering::Equal => (other.max_north, other.max_south),
            Ordering::Less => {
                let shift = other.shift_millis - shift_millis;
                (other.max_north >> shift, other.max_south >> shift)
            }
            Ordering::Greater => {
                let shift = shift_millis - other.shift_millis;
                (other.max_north << shift, other.max_south << shift)
            }
        };

        TcpSockSpeedLimitConfig {
            shift_millis,
            max_north: get_nonzero_smaller(self.max_north, other_north),
            max_south: get_nonzero_smaller(self.max_south, other_south),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tcp_sock_limit_shrink1() {
        let a = TcpSockSpeedLimitConfig {
            shift_millis: 10,
            max_north: 102400,
            max_south: 409600,
        };
        let b = TcpSockSpeedLimitConfig {
            shift_millis: 8,
            max_north: 12800,
            max_south: 204800,
        };
        let r = TcpSockSpeedLimitConfig {
            shift_millis: 10,
            max_north: 51200,
            max_south: 409600,
        };
        assert_eq!(a.shrink_as_smaller(&b), r);
    }

    #[test]
    fn tcp_sock_limit_shrink2() {
        let a = TcpSockSpeedLimitConfig {
            shift_millis: 10,
            max_north: 102400,
            max_south: 409600,
        };
        let b = TcpSockSpeedLimitConfig {
            shift_millis: 8,
            max_north: 12800,
            max_south: 204800,
        };
        let r = TcpSockSpeedLimitConfig {
            shift_millis: 8,
            max_north: 12800,
            max_south: 102400,
        };
        assert_eq!(b.shrink_as_smaller(&a), r);
    }
}
