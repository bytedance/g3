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

use anyhow::anyhow;
use percent_encoding::{AsciiSet, CONTROLS};

const USERNAME_MAX_LENGTH: usize = u8::MAX as usize;
const PASSWORD_MAX_LENGTH: usize = u8::MAX as usize;

const USER_INFO_PCT_ENCODING_SET: &AsciiSet = &CONTROLS
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'=')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'|');

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct Username {
    inner: String,
    len: u8,
}

impl Username {
    pub fn empty() -> Self {
        Username {
            inner: String::new(),
            len: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn len(&self) -> u8 {
        self.len
    }

    pub fn from_original(s: &str) -> anyhow::Result<Self> {
        if s.len() > USERNAME_MAX_LENGTH {
            return Err(anyhow!("too long string for a username"));
        }
        if s.contains(':') {
            return Err(anyhow!("colon character is not allowed"));
        }
        Ok(Username {
            inner: s.to_string(),
            len: s.len() as u8,
        })
    }

    pub fn from_encoded(s: &str) -> anyhow::Result<Self> {
        let decoded = percent_encoding::percent_decode_str(s)
            .decode_utf8()
            .map_err(|e| anyhow!("decode failed: {e}"))?;
        Username::from_original(decoded.as_ref())
    }

    pub fn as_original(&self) -> &str {
        &self.inner
    }

    pub fn to_encoded(&self) -> String {
        percent_encoding::utf8_percent_encode(self.as_original(), USER_INFO_PCT_ENCODING_SET)
            .to_string()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Password {
    inner: String,
    len: u8,
}

impl Password {
    pub fn empty() -> Self {
        Password {
            inner: String::new(),
            len: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn len(&self) -> u8 {
        self.len
    }

    pub fn from_original(s: &str) -> anyhow::Result<Self> {
        if s.len() > PASSWORD_MAX_LENGTH {
            return Err(anyhow!("too long string for a password"));
        }
        Ok(Password {
            inner: s.to_string(),
            len: s.len() as u8,
        })
    }

    pub fn from_encoded(s: &str) -> anyhow::Result<Self> {
        let decoded = percent_encoding::percent_decode_str(s)
            .decode_utf8()
            .map_err(|e| anyhow!("decode failed: {e}"))?;
        Password::from_original(decoded.as_ref())
    }

    pub fn as_original(&self) -> &str {
        &self.inner
    }

    pub fn to_encoded(&self) -> String {
        percent_encoding::utf8_percent_encode(self.as_original(), USER_INFO_PCT_ENCODING_SET)
            .to_string()
    }
}
