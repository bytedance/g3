/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod tcp;
mod udp;

pub const RATE_LIMIT_SHIFT_MILLIS_MAX: u8 = 12; // about 4s
pub const RATE_LIMIT_SHIFT_MILLIS_DEFAULT: u8 = 10;

pub use tcp::TcpSockSpeedLimitConfig;
pub use udp::UdpSockSpeedLimitConfig;

fn get_nonzero_smaller(a: usize, b: usize) -> usize {
    match (a, b) {
        (0, v) | (v, 0) => v,
        (left, right) => {
            if left > right {
                right
            } else {
                left
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_smaller_no_zero() {
        let a = 1usize;
        let b = 2usize;
        assert_eq!(get_nonzero_smaller(a, b), a);
    }

    #[test]
    fn get_smaller_with_zero() {
        let a = 0usize;
        let b = 1usize;
        assert_eq!(get_nonzero_smaller(a, b), b);
    }
}
