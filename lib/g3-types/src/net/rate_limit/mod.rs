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
