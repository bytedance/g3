/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

const CRYPT_HASH64: &[u8] = b"./0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

pub struct B64CryptEncoder {
    buf: Vec<u8>,
}

impl B64CryptEncoder {
    pub fn new(capacity: usize) -> Self {
        B64CryptEncoder {
            buf: Vec::<u8>::with_capacity(capacity),
        }
    }

    pub fn push<const LENGTH: usize>(&mut self, b2: u8, b1: u8, b0: u8) {
        let mut w: u32 = ((b2 as u32) << 16) | ((b1 as u32) << 8) | (b0 as u32);
        for _ in 0..LENGTH {
            self.buf.push(CRYPT_HASH64[w as usize & 0x3f]);
            w >>= 6;
        }
    }
}

impl From<B64CryptEncoder> for String {
    fn from(encoder: B64CryptEncoder) -> Self {
        unsafe { String::from_utf8_unchecked(encoder.buf) }
    }
}
