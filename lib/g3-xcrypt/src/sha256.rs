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

use std::str::FromStr;

use constant_time_eq::constant_time_eq_32;
use openssl::error::ErrorStack;
use openssl::md::Md;
use openssl::md_ctx::{MdCtx, MdCtxRef};

use super::{B64CryptDecoder, XCryptParseError, XCryptParseResult};

pub(super) const PREFIX: &str = "$5$";

const SALT_LEN_MAX: usize = 16;

const ROUNDS_DEFAULT: usize = 5000;
const ROUNDS_MIN: usize = 1000;
const ROUNDS_MAX: usize = 999999999;

const HASH_BIN_LEN: usize = 32;
const HASH_STR_LEN: usize = 43;

const ENCODE_INDEX_MAP: [u8; HASH_BIN_LEN] = [
    0, 10, 20, 21, 1, 11, 12, 22, 2, 3, 13, 23, 24, 4, 14, 15, 25, 5, 6, 16, 26, 27, 7, 17, 18, 28,
    8, 9, 19, 29, 31, 30,
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Sha256Crypt {
    rounds: usize,
    salt: String,
    hash: String,
    hash_bin: [u8; HASH_BIN_LEN],
}

fn sha256_update_recycled(
    md: &mut MdCtxRef,
    block: &[u8; HASH_BIN_LEN],
    len: usize,
) -> Result<(), ErrorStack> {
    let mut n = len;
    while n > HASH_BIN_LEN {
        md.digest_update(block)?;
        n -= HASH_BIN_LEN;
    }
    if n > 0 {
        md.digest_update(&block[0..n])?;
    }
    Ok(())
}

fn do_sha256_hash(
    phrase: &[u8],
    salt: &str,
    rounds: usize,
) -> Result<[u8; HASH_BIN_LEN], ErrorStack> {
    /*
      Compute alternate SHA256 sum with input PHRASE, SALT, and PHRASE.  The
      final result will be added to the first context.
    */
    let mut md = MdCtx::new()?;
    md.digest_init(Md::sha256())?;

    md.digest_update(phrase)?;
    md.digest_update(salt.as_bytes())?;
    md.digest_update(phrase)?;

    let mut hash = [0u8; HASH_BIN_LEN];
    md.digest_final(&mut hash)?;

    /* Prepare for the real work.  */
    md.digest_init(Md::sha256())?;

    md.digest_update(phrase)?;
    /*
      The last part is the salt string.  This must be at most 8
      characters, and it ends at the first `$' character (for
      compatibility with existing implementations).
    */
    md.digest_update(salt.as_bytes())?;

    /* Add for any character in the phrase one byte of the alternate sum.  */
    sha256_update_recycled(&mut md, &hash, phrase.len())?;

    /*
      Take the binary representation of the length of the phrase and for every
      1 add the alternate sum, for every 0 the phrase.
    */
    let mut plen = phrase.len();
    while plen > 0 {
        if (plen & 1) == 0 {
            md.digest_update(phrase)?;
        } else {
            md.digest_update(&hash)?;
        }
        plen >>= 1;
    }

    /* Create intermediate result.  */
    md.digest_final(&mut hash)?;

    /* Start computation of P byte sequence.  */
    md.digest_init(Md::sha256())?;
    /* For every character in the password add the entire password.  */
    for _ in 0..phrase.len() {
        md.digest_update(phrase)?;
    }
    let mut p_bytes = [0u8; HASH_BIN_LEN];
    md.digest_final(&mut p_bytes)?;

    /* Start computation of S byte sequence.  */
    md.digest_init(Md::sha256())?;
    for _ in 0..(hash[0] as usize + 16) {
        md.digest_update(salt.as_bytes())?;
    }
    let mut s_bytes = [0u8; HASH_BIN_LEN];
    md.digest_final(&mut s_bytes)?;

    for r in 0..rounds {
        md.digest_init(Md::sha256())?;

        /* Add phrase or last result.  */
        if (r & 1) == 0 {
            md.digest_update(&hash)?;
        } else {
            sha256_update_recycled(&mut md, &p_bytes, phrase.len())?;
        }

        /* Add salt for numbers not divisible by 3.  */
        if (r % 3) != 0 {
            sha256_update_recycled(&mut md, &s_bytes, salt.len())?;
        }

        /* Add phrase for numbers not divisible by 7.  */
        if (r % 7) != 0 {
            sha256_update_recycled(&mut md, &p_bytes, phrase.len())?;
        }

        /* Add phrase or last result.  */
        if (r & 1) == 0 {
            sha256_update_recycled(&mut md, &p_bytes, phrase.len())?;
        } else {
            md.digest_update(&hash)?;
        }

        md.digest_final(&mut hash)?;
    }

    Ok(hash)
}

impl Sha256Crypt {
    pub(super) fn parse(v: &str) -> XCryptParseResult<Self> {
        let mut rounds = ROUNDS_DEFAULT;
        let mut s = v;
        if let Some(r) = v.strip_prefix("rounds=") {
            if let Some(d) = memchr::memchr(b'$', r.as_bytes()) {
                if d == 0 {
                    return Err(XCryptParseError::InvalidRounds);
                }

                rounds = usize::from_str(&r[0..d]).map_err(|_| XCryptParseError::InvalidRounds)?;
                if (ROUNDS_MIN..=ROUNDS_MAX).contains(&rounds) {
                    return Err(XCryptParseError::OutOfRangeRounds);
                }

                s = &r[d + 1..];
            } else {
                return Err(XCryptParseError::InvalidRounds);
            }
        }

        if let Some(d) = memchr::memchr(b'$', s.as_bytes()) {
            if d == 0 {
                return Err(XCryptParseError::NoSaltFound);
            }

            if d > SALT_LEN_MAX {
                return Err(XCryptParseError::SaltTooLong);
            }
            if d + 1 + HASH_STR_LEN != s.len() {
                return Err(XCryptParseError::InvalidHashSize);
            }

            let hash = &v.as_bytes()[d + 1..];
            let mut bin = [0u8; HASH_BIN_LEN];
            for i in 0..10 {
                let ii = i * 4;
                let oi = i * 3;
                B64CryptDecoder::decode_buf(&hash[ii..ii + 4], &mut bin[oi..oi + 3]);
            }
            let r = B64CryptDecoder::decode(hash[40], hash[41], hash[42], 0);
            bin[30] = r.1;
            bin[31] = r.2;

            let mut hash_bin = [0u8; HASH_BIN_LEN];
            for i in 0..HASH_BIN_LEN {
                let j = ENCODE_INDEX_MAP[i];
                hash_bin[j as usize] = bin[i];
            }

            Ok(Sha256Crypt {
                rounds,
                salt: s[0..d].to_string(),
                hash: s[d + 1..].to_string(),
                hash_bin,
            })
        } else {
            Err(XCryptParseError::NoSaltFound)
        }
    }

    pub(super) fn verify(&self, phrase: &[u8]) -> Result<bool, ErrorStack> {
        do_sha256_hash(phrase, &self.salt, self.rounds)
            .map(|hash| constant_time_eq_32(&hash, &self.hash_bin))
    }
}
