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

use std::cell::RefCell;

use anyhow::anyhow;
use digest::Digest;
use md5::Md5;
use sha1::Sha1;

const SALT_LENGTH: usize = 8;
const MD5_LENGTH: usize = 16;
const SHA1_LENGTH: usize = 20;

thread_local! {
    static HASH_TL_BUF: RefCell<Vec<u8>> = RefCell::new(Vec::with_capacity(256));
}

#[derive(Clone)]
enum HashValue {
    Md5([u8; MD5_LENGTH]),
    Sha1([u8; SHA1_LENGTH]),
    Blake3(blake3::Hash),
}

impl HashValue {
    fn hash_match(&self, buf: &[u8]) -> bool {
        match self {
            HashValue::Md5(v) => {
                let md5 = Md5::digest(buf);
                v.eq(md5.as_slice())
            }
            HashValue::Sha1(v) => {
                let sha1 = Sha1::digest(buf);
                v.eq(sha1.as_slice())
            }
            HashValue::Blake3(v) => {
                let b3 = blake3::hash(buf);
                v.eq(&b3)
            }
        }
    }
}

/// A fast hashed passphrase type which is weak for brute forces but fast to verify
/// we use dual hash here to reduce the chance of password collision.
/// Note that the weakness is the same as md5 if the attackers try to brute force it.
#[derive(Clone)]
pub struct FastHashedPassPhrase {
    salt: [u8; SALT_LENGTH],
    values: Vec<HashValue>,
}

impl FastHashedPassPhrase {
    pub fn new(salt: &str) -> anyhow::Result<Self> {
        let salt_vec = hex::decode(salt).map_err(|_| anyhow!("invalid salt hex string"))?;
        if salt_vec.len() != SALT_LENGTH {
            return Err(anyhow!("invalid length for salt"));
        }
        let mut salt = [0u8; SALT_LENGTH];
        salt.copy_from_slice(salt_vec.as_slice());

        Ok(FastHashedPassPhrase {
            salt,
            values: Vec::with_capacity(2),
        })
    }

    pub fn push_md5(&mut self, s: &str) -> anyhow::Result<()> {
        let md5_vec = hex::decode(s).map_err(|_| anyhow!("invalid md5 hex string"))?;
        if md5_vec.len() != MD5_LENGTH {
            return Err(anyhow!("invalid length for md5"));
        }
        let mut md5 = [0u8; MD5_LENGTH];
        md5.copy_from_slice(md5_vec.as_slice());

        self.values.push(HashValue::Md5(md5));

        Ok(())
    }

    pub fn push_sha1(&mut self, s: &str) -> anyhow::Result<()> {
        let sha1_vec = hex::decode(s).map_err(|_| anyhow!("invalid sha1 hex string"))?;
        if sha1_vec.len() != SHA1_LENGTH {
            return Err(anyhow!("invalid length for sha1"));
        }
        let mut sha1 = [0u8; SHA1_LENGTH];
        sha1.copy_from_slice(sha1_vec.as_slice());

        self.values.push(HashValue::Sha1(sha1));

        Ok(())
    }

    pub fn push_blake3(&mut self, s: &str) -> anyhow::Result<()> {
        let b3 =
            blake3::Hash::from_hex(s).map_err(|e| anyhow!("invalid blake3 hex string: {}", e))?;

        self.values.push(HashValue::Blake3(b3));

        Ok(())
    }

    pub fn verify(&self, pass: &str) -> bool {
        HASH_TL_BUF.with(|buf| {
            let mut buf = buf.borrow_mut();
            buf.extend_from_slice(pass.as_bytes());
            buf.extend_from_slice(&self.salt);

            let mut all_verified = true;
            for hv in self.values.iter() {
                if !hv.hash_match(buf.as_slice()) {
                    all_verified = false;
                    break;
                }
            }
            buf.clear();
            all_verified
        })
    }

    pub fn check_config(&self) -> anyhow::Result<()> {
        if self.values.is_empty() {
            return Err(anyhow!("no hash is set"));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_ok() {
        let mut p = FastHashedPassPhrase::new("d950eeffd53f7189").unwrap();
        p.push_md5("28cb2d22a1148a2c4c43d2c8eab0a202").unwrap();
        p.push_sha1("0b39e984b59251425245e81241aebf7dbe197cc3")
            .unwrap();

        assert!(p.verify("IQ5ZhanWaop2cw"));
    }
}
