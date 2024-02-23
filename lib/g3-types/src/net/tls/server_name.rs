/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use std::str::Utf8Error;
use std::{fmt, str};

use thiserror::Error;

use crate::net::Host;

#[derive(Debug, Error)]
pub enum TlsServerNameError {
    #[error("not enough data: {0}")]
    NotEnoughData(usize),
    #[error("invalid list length {0}")]
    InvalidListLength(u16),
    #[error("invalid name type {0}")]
    InvalidNameType(u8),
    #[error("invalid name length {0}")]
    InvalidNameLength(usize),
    #[error("invalid host name: {0}")]
    InvalidHostName(Utf8Error),
}

#[derive(Clone)]
pub struct TlsServerName {
    host_name: String,
}

impl TlsServerName {
    pub fn from_extension_value(buf: &[u8]) -> Result<TlsServerName, TlsServerNameError> {
        let buf_len = buf.len();
        if buf_len < 5 {
            return Err(TlsServerNameError::NotEnoughData(buf_len));
        }

        let list_len = u16::from_be_bytes([buf[0], buf[1]]);
        if list_len as usize + 2 != buf_len {
            return Err(TlsServerNameError::InvalidListLength(list_len));
        }

        let name_type = buf[2];
        if name_type != 0x00 {
            return Err(TlsServerNameError::InvalidNameType(name_type));
        }

        let name_len = u16::from_be_bytes([buf[3], buf[4]]) as usize;
        if name_len + 5 > buf_len {
            return Err(TlsServerNameError::InvalidNameLength(name_len));
        }

        let name = &buf[5..5 + name_len];
        let host_name = str::from_utf8(name).map_err(TlsServerNameError::InvalidHostName)?;

        Ok(TlsServerName {
            host_name: host_name.to_string(),
        })
    }
}

impl AsRef<str> for TlsServerName {
    fn as_ref(&self) -> &str {
        self.host_name.as_str()
    }
}

impl From<TlsServerName> for Host {
    fn from(value: TlsServerName) -> Self {
        Host::Domain(value.host_name)
    }
}

impl From<&TlsServerName> for Host {
    fn from(value: &TlsServerName) -> Self {
        Host::Domain(value.host_name.to_string())
    }
}

impl fmt::Display for TlsServerName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.host_name)
    }
}
