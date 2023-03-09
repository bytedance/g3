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

mod b64;
pub(crate) use b64::B64CryptEncoder;

mod error;
pub use error::{XCryptParseError, XCryptParseResult};

mod md5;
mod sha256;
mod sha512;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum XCryptHash {
    Md5(md5::Md5Crypt),
    Sha256(sha256::Sha256Crypt),
    Sha512(sha512::Sha512Crypt),
}

impl XCryptHash {
    pub fn parse(v: &str) -> XCryptParseResult<Self> {
        if let Some(s) = v.strip_prefix(crate::md5::PREFIX) {
            let v = crate::md5::Md5Crypt::parse(s)?;
            Ok(XCryptHash::Md5(v))
        } else if let Some(s) = v.strip_prefix(sha256::PREFIX) {
            let v = sha256::Sha256Crypt::parse(s)?;
            Ok(XCryptHash::Sha256(v))
        } else if let Some(s) = v.strip_prefix(sha512::PREFIX) {
            let v = sha512::Sha512Crypt::parse(s)?;
            Ok(XCryptHash::Sha512(v))
        } else {
            Err(XCryptParseError::UnknownPrefix)
        }
    }

    pub fn verify(&self, phrase: &[u8]) -> bool {
        match self {
            XCryptHash::Md5(this) => this.verify(phrase),
            XCryptHash::Sha256(this) => this.verify(phrase),
            XCryptHash::Sha512(this) => this.verify(phrase),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md5() {
        let crypt = XCryptHash::parse("$1$DDiGYGte$K/SAC4VvllDonGcP1EfaY1").unwrap();
        assert!(crypt.verify("123456".as_bytes()));
    }

    #[test]
    fn sha256() {
        let crypt =
            XCryptHash::parse("$5$W9wFmTCpBILzJn18$X496nPJHVQ895fwotE3WPBLmxgxGD8ivpUhfmoKbtb7")
                .unwrap();
        assert!(crypt.verify("123456".as_bytes()));
    }

    #[test]
    fn sha512() {
        let s = "$6$yeDpErl4xq9E2vKP$\
            .reNyfNzRJyAJrlh38J1XGx/5QTfBy3IedVNdTqfWqSeZFPAbXzV85uNK9fdmXvGCxizHVcAiIoQ4uXMJWuB6/";
        let crypt = XCryptHash::parse(s).unwrap();
        assert!(crypt.verify("123456".as_bytes()));
    }
}
