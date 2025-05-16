/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use constant_time_eq::constant_time_eq_16;
use openssl::error::ErrorStack;
use openssl::md::Md;
use openssl::md_ctx::MdCtx;

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

fn do_md5_hash(phrase: &[u8], salt: &str) -> Result<[u8; HASH_BIN_LEN], ErrorStack> {
    /*
      Compute alternate MD5 sum with input PHRASE, SALT, and PHRASE.  The
      final result will be added to the first context.
    */
    let mut md = MdCtx::new()?;
    md.digest_init(Md::md5())?;

    md.digest_update(phrase)?;
    md.digest_update(salt.as_bytes())?;
    md.digest_update(phrase)?;

    let mut hash = [0u8; HASH_BIN_LEN];
    md.digest_final(&mut hash)?;

    /* Prepare for the real work.  */
    md.digest_init(Md::md5())?;

    md.digest_update(phrase)?;
    md.digest_update(PREFIX.as_bytes())?;
    md.digest_update(salt.as_bytes())?;

    /* Add for any character in the phrase one byte of the alternate sum.  */
    let mut plen = phrase.len();
    while plen > HASH_BIN_LEN {
        md.digest_update(&hash)?;
        plen -= HASH_BIN_LEN;
    }
    if plen > 0 {
        md.digest_update(&hash[0..plen])?;
    }

    /*
      The original implementation now does something weird: for every 1
      bit in the phrase the first 0 is added to the buffer, for every 0
      bit the first character of the phrase.  This does not seem to be
      what was intended, but we have to follow this to be compatible.
    */
    plen = phrase.len();
    while plen > 0 {
        if (plen & 1) == 0 {
            md.digest_update(&phrase[..1])?;
        } else {
            md.digest_update(&[0u8])?;
        }
        plen >>= 1;
    }

    /* Create intermediate result.  */
    md.digest_final(&mut hash)?;

    for r in 0..1000 {
        md.digest_init(Md::md5())?;

        /* Add phrase or last result.  */
        if (r & 1) == 0 {
            md.digest_update(&hash)?;
        } else {
            md.digest_update(phrase)?;
        }

        /* Add salt for numbers not divisible by 3.  */
        if (r % 3) != 0 {
            md.digest_update(salt.as_bytes())?;
        }

        /* Add phrase for numbers not divisible by 7.  */
        if (r % 7) != 0 {
            md.digest_update(phrase)?;
        }

        /* Add phrase or last result.  */
        if (r & 1) == 0 {
            md.digest_update(phrase)?;
        } else {
            md.digest_update(&hash)?;
        }

        /* Create intermediate result.  */
        md.digest_final(&mut hash)?;
    }

    Ok(hash)
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

    pub(super) fn verify(&self, phrase: &[u8]) -> Result<bool, ErrorStack> {
        do_md5_hash(phrase, &self.salt).map(|hash| constant_time_eq_16(&hash, &self.hash_bin))
    }
}
