/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

pub trait ToZigZag<T> {
    fn to_zig_zag(self) -> T;
}

pub trait FromZigZag<T> {
    fn from_zig_zag(value: T) -> Self;
}

impl ToZigZag<u32> for i32 {
    fn to_zig_zag(self) -> u32 {
        ((self >> 31) ^ (self << 1)) as u32
    }
}

impl FromZigZag<u32> for i32 {
    fn from_zig_zag(value: u32) -> Self {
        (value >> 1) as i32 ^ -(value as i32 & 1)
    }
}

impl ToZigZag<u64> for i64 {
    fn to_zig_zag(self) -> u64 {
        ((self >> 63) ^ (self << 1)) as u64
    }
}

impl FromZigZag<u64> for i64 {
    fn from_zig_zag(value: u64) -> Self {
        (value >> 1) as i64 ^ -(value as i64 & 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_i32() {
        assert_eq!(0i32.to_zig_zag(), 0);
        assert_eq!((-1i32).to_zig_zag(), 1);
        assert_eq!(1i32.to_zig_zag(), 2);
        assert_eq!((-2i32).to_zig_zag(), 3);
        assert_eq!(2i32.to_zig_zag(), 4);
        assert_eq!(i32::MAX.to_zig_zag(), u32::MAX - 1);
        assert_eq!(i32::MIN.to_zig_zag(), u32::MAX);
    }

    #[test]
    fn decode_u32() {
        assert_eq!(i32::from_zig_zag(0), 0);
        assert_eq!(i32::from_zig_zag(1), -1);
        assert_eq!(i32::from_zig_zag(2), 1);
        assert_eq!(i32::from_zig_zag(3), -2);
        assert_eq!(i32::from_zig_zag(4), 2);
        assert_eq!(i32::from_zig_zag(u32::MAX - 1), i32::MAX);
        assert_eq!(i32::from_zig_zag(u32::MAX), i32::MIN);
    }

    #[test]
    fn encode_i64() {
        assert_eq!(0i64.to_zig_zag(), 0);
        assert_eq!((-1i64).to_zig_zag(), 1);
        assert_eq!(1i64.to_zig_zag(), 2);
        assert_eq!((-2i64).to_zig_zag(), 3);
        assert_eq!(2i64.to_zig_zag(), 4);
        assert_eq!(i64::MAX.to_zig_zag(), u64::MAX - 1);
        assert_eq!(i64::MIN.to_zig_zag(), u64::MAX);
    }

    #[test]
    fn decode_u64() {
        assert_eq!(i64::from_zig_zag(0), 0);
        assert_eq!(i64::from_zig_zag(1), -1);
        assert_eq!(i64::from_zig_zag(2), 1);
        assert_eq!(i64::from_zig_zag(3), -2);
        assert_eq!(i64::from_zig_zag(4), 2);
        assert_eq!(i64::from_zig_zag(u64::MAX - 1), i64::MAX);
        assert_eq!(i64::from_zig_zag(u64::MAX), i64::MIN);
    }
}
