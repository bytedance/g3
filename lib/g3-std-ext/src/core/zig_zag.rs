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

macro_rules! impl_zig_zag {
    ($it:ty, $ut:ty) => {
        impl ToZigZag<$ut> for $it {
            fn to_zig_zag(self) -> $ut {
                <$it>::cast_unsigned((self >> (<$it>::BITS - 1)) ^ (self << 1))
            }
        }

        impl FromZigZag<$ut> for $it {
            fn from_zig_zag(value: $ut) -> Self {
                <$ut>::cast_signed(value >> 1) ^ -<$ut>::cast_signed(value & 1)
            }
        }
    };
}

impl_zig_zag!(i16, u16);
impl_zig_zag!(i32, u32);
impl_zig_zag!(i64, u64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_i16() {
        assert_eq!(0i16.to_zig_zag(), 0);
        assert_eq!((-1i16).to_zig_zag(), 1);
        assert_eq!(1i16.to_zig_zag(), 2);
        assert_eq!((-2i16).to_zig_zag(), 3);
        assert_eq!(2i32.to_zig_zag(), 4);
        assert_eq!(i16::MAX.to_zig_zag(), u16::MAX - 1);
        assert_eq!(i16::MIN.to_zig_zag(), u16::MAX);
    }

    #[test]
    fn decode_u16() {
        assert_eq!(i16::from_zig_zag(0), 0);
        assert_eq!(i16::from_zig_zag(1), -1);
        assert_eq!(i16::from_zig_zag(2), 1);
        assert_eq!(i16::from_zig_zag(3), -2);
        assert_eq!(i16::from_zig_zag(4), 2);
        assert_eq!(i16::from_zig_zag(u16::MAX - 1), i16::MAX);
        assert_eq!(i16::from_zig_zag(u16::MAX), i16::MIN);
    }

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
