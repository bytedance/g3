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

use std::convert::From;
use std::fmt;

#[derive(PartialOrd, PartialEq, Ord, Eq)]
pub enum SocksAuthMethod {
    None,
    GssApi,
    User,
    Chap,
    OtherAssigned(u8),
    Private(u8),
    NoAcceptable,
}

impl SocksAuthMethod {
    pub(crate) fn code(&self) -> u8 {
        match self {
            SocksAuthMethod::None => 0x00,
            SocksAuthMethod::GssApi => 0x01,
            SocksAuthMethod::User => 0x02,
            SocksAuthMethod::Chap => 0x03,
            SocksAuthMethod::OtherAssigned(v) => *v,
            SocksAuthMethod::Private(v) => *v,
            SocksAuthMethod::NoAcceptable => 0xFF,
        }
    }
}

impl fmt::Display for SocksAuthMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SocksAuthMethod::None => write!(f, "None"),
            SocksAuthMethod::GssApi => write!(f, "GssApi"),
            SocksAuthMethod::User => write!(f, "User"),
            SocksAuthMethod::Chap => write!(f, "Chap"),
            SocksAuthMethod::OtherAssigned(v) => write!(f, "OtherAssigned({v})"),
            SocksAuthMethod::Private(v) => write!(f, "Private({v})"),
            SocksAuthMethod::NoAcceptable => write!(f, "NoAcceptable"),
        }
    }
}

impl From<u8> for SocksAuthMethod {
    fn from(method: u8) -> Self {
        match method {
            0x00 => Self::None,
            0x01 => Self::GssApi,
            0x02 => Self::User,
            0x03 => Self::Chap,
            v if method <= 0x7F => Self::OtherAssigned(v),
            v if method < 0xFF => Self::Private(v),
            _ => Self::NoAcceptable, // 0xFF
        }
    }
}
