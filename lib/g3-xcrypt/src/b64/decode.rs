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

#[rustfmt::skip]
const B64_CRYPT_DIGITS: [u8; 256] = [
//  x0    x1   x2   x3   x4   x5   x6   x7   x8   x9   xA   xB   xC   xD   xE   xF
    0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0, // 0x
    0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0, // 1x
    0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   1, // 2x
    2,   3,   4,   5,   6,   7,   8,   9,  10,  11,   0,   0,   0,   0,   0,   0, // 3x
    0,  12,  13,  14,  15,  16,  17,  18,  19,  20,  21,  22,  23,  24,  25,  26, // 4x
   27,  28,  29,  30,  31,  32,  33,  34,  35,  36,  37,   0,   0,   0,   0,   0, // 5x
    0,  38,  39,  40,  41,  42,  43,  44,  45,  46,  47,  48,  49,  50,  51,  52, // 6x
   53,  54,  55,  56,  57,  58,  59,  60,  61,  62,  63,   0,   0,   0,   0,   0, // 7x
    0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0, // 8x
    0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0, // 9x
    0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0, // Ax
    0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0, // Bx
    0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0, // Cx
    0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0, // Dx
    0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0, // Ex
    0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0,   0, // Fx
];

pub(crate) struct B64CryptDecoder {}

const fn b64_char_to_u32(c: u8) -> u32 {
    B64_CRYPT_DIGITS[c as usize] as u32
}

impl B64CryptDecoder {
    pub(crate) fn decode_buf(input: &[u8], output: &mut [u8]) {
        let c = b64_char_to_u32(input[0])
            | (b64_char_to_u32(input[1]) << 6)
            | (b64_char_to_u32(input[2]) << 12)
            | (b64_char_to_u32(input[3]) << 18);
        output[0] = ((c >> 16) & 0xFF) as u8;
        output[1] = ((c >> 8) & 0xFF) as u8;
        output[2] = (c & 0xFF) as u8;
    }

    pub(crate) fn decode(c1: u8, c2: u8, c3: u8, c4: u8) -> (u8, u8, u8) {
        let c = b64_char_to_u32(c1)
            | (b64_char_to_u32(c2) << 6)
            | (b64_char_to_u32(c3) << 12)
            | (b64_char_to_u32(c4) << 18);
        let b2 = (c >> 16) & 0xFF;
        let b1 = (c >> 8) & 0xFF;
        let b0 = c & 0xFF;
        (b2 as u8, b1 as u8, b0 as u8)
    }
}
