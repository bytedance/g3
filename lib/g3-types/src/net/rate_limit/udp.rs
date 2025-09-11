/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::cmp::Ordering;

use anyhow::anyhow;

use super::{RATE_LIMIT_SHIFT_MILLIS_MAX, get_nonzero_smaller};

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
        if self.shift_millis > RATE_LIMIT_SHIFT_MILLIS_MAX {
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

#[cfg(test)]
mod tests {
    use super::*;

    // Helper function to create a basic config for testing
    fn basic_config() -> UdpSockSpeedLimitConfig {
        UdpSockSpeedLimitConfig {
            shift_millis: 8,
            max_north_packets: 1000,
            max_south_packets: 2000,
            max_north_bytes: 1024000,
            max_south_bytes: 2048000,
        }
    }

    #[test]
    fn validate_config() {
        let config = basic_config();
        assert!(config.validate().is_ok());

        let config = UdpSockSpeedLimitConfig {
            shift_millis: RATE_LIMIT_SHIFT_MILLIS_MAX,
            ..basic_config()
        };
        assert!(config.validate().is_ok());

        let config = UdpSockSpeedLimitConfig {
            shift_millis: RATE_LIMIT_SHIFT_MILLIS_MAX + 1,
            ..basic_config()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn shrink_as_smaller_config() {
        // self shift_millis == 0
        let config_zero = UdpSockSpeedLimitConfig {
            shift_millis: 0,
            ..Default::default()
        };
        let other = basic_config();
        let result = config_zero.shrink_as_smaller(&other);
        assert_eq!(result, other);

        // other shift_millis == 0
        let config = basic_config();
        let other_zero = UdpSockSpeedLimitConfig {
            shift_millis: 0,
            ..Default::default()
        };
        let result = config.shrink_as_smaller(&other_zero);
        assert_eq!(result, config);

        // equal shift
        let config1 = UdpSockSpeedLimitConfig {
            shift_millis: 5,
            max_north_packets: 1000,
            max_south_packets: 800,
            max_north_bytes: 2000000,
            max_south_bytes: 1500000,
        };
        let config2 = UdpSockSpeedLimitConfig {
            shift_millis: 5,
            max_north_packets: 1200,
            max_south_packets: 600,
            max_north_bytes: 1800000,
            max_south_bytes: 1800000,
        };
        let result = config1.shrink_as_smaller(&config2);

        assert_eq!(result.shift_millis, 5);
        assert_eq!(result.max_north_packets, 1000);
        assert_eq!(result.max_south_packets, 600);
        assert_eq!(result.max_north_bytes, 1800000);
        assert_eq!(result.max_south_bytes, 1500000);

        // less shift
        let config1 = UdpSockSpeedLimitConfig {
            shift_millis: 6,
            max_north_packets: 1000,
            max_south_packets: 2000,
            max_north_bytes: 1024000,
            max_south_bytes: 2048000,
        };
        let config2 = UdpSockSpeedLimitConfig {
            shift_millis: 8,          // 2 bits more than config1
            max_north_packets: 4000,  // will be >> 2 = 1000
            max_south_packets: 6000,  // will be >> 2 = 1500
            max_north_bytes: 8192000, // will be >> 2 = 2048000
            max_south_bytes: 4096000, // will be >> 2 = 1024000
        };
        let result = config1.shrink_as_smaller(&config2);

        assert_eq!(result.shift_millis, 6);
        assert_eq!(result.max_north_packets, 1000);
        assert_eq!(result.max_south_packets, 1500);
        assert_eq!(result.max_north_bytes, 1024000);
        assert_eq!(result.max_south_bytes, 1024000);

        // greater shift
        let config1 = UdpSockSpeedLimitConfig {
            shift_millis: 8,
            max_north_packets: 1000,
            max_south_packets: 2000,
            max_north_bytes: 1024000,
            max_south_bytes: 2048000,
        };
        let config2 = UdpSockSpeedLimitConfig {
            shift_millis: 6,         // 2 bits less than config1
            max_north_packets: 250,  // will be << 2 = 1000
            max_south_packets: 400,  // will be << 2 = 1600
            max_north_bytes: 200000, // will be << 2 = 800000
            max_south_bytes: 600000, // will be << 2 = 2400000
        };
        let result = config1.shrink_as_smaller(&config2);

        assert_eq!(result.shift_millis, 8);
        assert_eq!(result.max_north_packets, 1000);
        assert_eq!(result.max_south_packets, 1600);
        assert_eq!(result.max_north_bytes, 800000);
        assert_eq!(result.max_south_bytes, 2048000);

        // with zero values
        let config1 = UdpSockSpeedLimitConfig {
            shift_millis: 5,
            max_north_packets: 0,
            max_south_packets: 1000,
            max_north_bytes: 500000,
            max_south_bytes: 0,
        };
        let config2 = UdpSockSpeedLimitConfig {
            shift_millis: 5,
            max_north_packets: 800,
            max_south_packets: 0,
            max_north_bytes: 0,
            max_south_bytes: 600000,
        };
        let result = config1.shrink_as_smaller(&config2);

        assert_eq!(result.shift_millis, 5);
        assert_eq!(result.max_north_packets, 800);
        assert_eq!(result.max_south_packets, 1000);
        assert_eq!(result.max_north_bytes, 500000);
        assert_eq!(result.max_south_bytes, 600000);
    }
}
