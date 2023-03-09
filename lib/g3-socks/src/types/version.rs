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

use std::convert::TryFrom;
use std::fmt;

use super::SocksNegotiationError;

#[derive(Debug)]
pub enum SocksVersion {
    V4a = 0x04,
    V5 = 0x05,
    V6 = 0x06,
}

impl SocksVersion {
    pub fn code(&self) -> u8 {
        match self {
            SocksVersion::V4a => 0x04,
            SocksVersion::V5 => 0x05,
            SocksVersion::V6 => 0x06,
        }
    }
}

impl fmt::Display for SocksVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SocksVersion::V4a => write!(f, "socks v4(a)"),
            SocksVersion::V5 => write!(f, "socks v5"),
            SocksVersion::V6 => write!(f, "socks v6"),
        }
    }
}

impl TryFrom<u8> for SocksVersion {
    type Error = SocksNegotiationError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x04 => Ok(SocksVersion::V4a),
            0x05 => Ok(SocksVersion::V5),
            0x06 => Ok(SocksVersion::V6),
            _ => Err(SocksNegotiationError::InvalidVersion),
        }
    }
}
