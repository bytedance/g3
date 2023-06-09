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

use digest::Digest;
use md5::Md5;

use super::{B64CryptDecoder, XCryptParseError, XCryptParseResult};

pub(super) const PREFIX: &str = "$1$";

const SALT_LEN_MAX: usize = 8;

const HASH_BIN_LEN: usize = 16;
const HASH_STR_LEN: usize = 22;

const ENCODE_INDEX_MAP: [u8; HASH_BIN_LEN] = [0, 6, 12, 1, 7, 13, 2, 8, 14, 3, 9, 15, 4, 10, 5, 11];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Md5Crypt {
    salt: String,
    hash: String,
    hash_bin: [u8; HASH_BIN_LEN],
}

fn do_md5_hash(phrase: &[u8], salt: &str) -> [u8; HASH_BIN_LEN] {
    /*
      Compute alternate MD5 sum with input PHRASE, SALT, and PHRASE.  The
      final result will be added to the first context.
    */
    let mut digest = Md5::new();

    digest.update(phrase);
    digest.update(salt.as_bytes());
    digest.update(phrase);

    let hash = digest.finalize(); // the results should be of HASH_BIN_LEN bytes

    /* Prepare for the real work.  */
    let mut digest = Md5::new();

    digest.update(phrase);
    digest.update(PREFIX.as_bytes());
    digest.update(salt.as_bytes());

    /* Add for any character in the phrase one byte of the alternate sum.  */
    let mut plen = phrase.len();
    while plen > HASH_BIN_LEN {
        digest.update(hash);
        plen -= HASH_BIN_LEN;
    }
    if plen > 0 {
        digest.update(&hash[0..plen]);
    }

    /*
      The original implementation now does something weird: for every 1
      bit in the phrase the first 0 is added to the buffer, for every 0
      bit the first character of the phrase.  This does not seem to be
      what was intended but we have to follow this to be compatible.
    */
    plen = phrase.len();
    while plen > 0 {
        if (plen & 1) == 0 {
            digest.update(&phrase[..1]);
        } else {
            digest.update([0u8]);
        }
        plen >>= 1;
    }

    /* Create intermediate result.  */
    let mut hash = digest.finalize();

    for r in 0..1000 {
        let mut digest = Md5::new();

        /* Add phrase or last result.  */
        if (r & 1) == 0 {
            digest.update(hash);
        } else {
            digest.update(phrase);
        }

        /* Add salt for numbers not divisible by 3.  */
        if (r % 3) != 0 {
            digest.update(salt.as_bytes());
        }

        /* Add phrase for numbers not divisible by 7.  */
        if (r % 7) != 0 {
            digest.update(phrase);
        }

        /* Add phrase or last result.  */
        if (r & 1) == 0 {
            digest.update(phrase);
        } else {
            digest.update(hash);
        }

        /* Create intermediate result.  */
        hash = digest.finalize();
    }

    hash.into()

    /*
    let mut encoder = B64CryptEncoder::new(HASH_STR_LEN);
    encoder.push::<4>(hash[0], hash[6], hash[12]);
    encoder.push::<4>(hash[1], hash[7], hash[13]);
    encoder.push::<4>(hash[2], hash[8], hash[14]);
    encoder.push::<4>(hash[3], hash[9], hash[15]);
    encoder.push::<4>(hash[4], hash[10], hash[5]);
    encoder.push::<2>(0, 0, hash[11]);

    encoder.into()
    */
}

impl Md5Crypt {
    pub(super) fn parse(v: &str) -> XCryptParseResult<Self> {
        if let Some(d) = memchr::memchr(b'$', v.as_bytes()) {
            if d == 0 {
                return Err(XCryptParseError::NoSaltFound);
            }
            if d > SALT_LEN_MAX {
                return Err(XCryptParseError::SaltTooLong);
            }
            if d + 1 + HASH_STR_LEN != v.len() {
                return Err(XCryptParseError::InvalidHashSize);
            }

            let hash = &v.as_bytes()[d + 1..];
            let mut bin = [0u8; HASH_BIN_LEN];
            for i in 0..5 {
                let ii = i * 4;
                let oi = i * 3;
                B64CryptDecoder::decode_buf(&hash[ii..ii + 4], &mut bin[oi..oi + 3]);
            }
            let r = B64CryptDecoder::decode(hash[20], hash[21], 0, 0);
            bin[15] = r.2;

            let mut hash_bin = [0u8; HASH_BIN_LEN];
            for i in 0..HASH_BIN_LEN {
                let j = ENCODE_INDEX_MAP[i];
                hash_bin[j as usize] = bin[i];
            }

            Ok(Md5Crypt {
                salt: v[0..d].to_string(),
                hash: v[d + 1..].to_string(),
                hash_bin,
            })
        } else {
            Err(XCryptParseError::NoSaltFound)
        }
    }

    pub(super) fn verify(&self, phrase: &[u8]) -> bool {
        let hash = do_md5_hash(phrase, &self.salt);
        self.hash_bin.eq(&hash)
    }
}
