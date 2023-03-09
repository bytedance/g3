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

const CRYPT_HASH64: &[u8] = b"./0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

pub(crate) struct B64CryptEncoder {
    buf: Vec<u8>,
}

impl B64CryptEncoder {
    pub(crate) fn new(capacity: usize) -> Self {
        B64CryptEncoder {
            buf: Vec::<u8>::with_capacity(capacity),
        }
    }

    pub(crate) fn push(&mut self, b2: u8, b1: u8, b0: u8, len: usize) {
        let mut w: u32 = ((b2 as u32) << 16) | ((b1 as u32) << 8) | (b0 as u32);

        let mut n = len;
        while n > 0 {
            self.buf.push(CRYPT_HASH64[w as usize & 0x3f]);
            w >>= 6;
            n -= 1;
        }
    }
}

impl From<B64CryptEncoder> for String {
    fn from(encoder: B64CryptEncoder) -> Self {
        unsafe { String::from_utf8_unchecked(encoder.buf) }
    }
}
