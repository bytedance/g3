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
