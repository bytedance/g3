/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::fmt;

#[derive(PartialOrd, PartialEq, Ord, Eq, Debug)]
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
            SocksAuthMethod::None => f.write_str("None"),
            SocksAuthMethod::GssApi => f.write_str("GssApi"),
            SocksAuthMethod::User => f.write_str("User"),
            SocksAuthMethod::Chap => f.write_str("Chap"),
            SocksAuthMethod::OtherAssigned(v) => write!(f, "OtherAssigned({v})"),
            SocksAuthMethod::Private(v) => write!(f, "Private({v})"),
            SocksAuthMethod::NoAcceptable => f.write_str("NoAcceptable"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operations() {
        for (code, display) in [
            (0x00, "None"),
            (0x01, "GssApi"),
            (0x02, "User"),
            (0x03, "Chap"),
            (0x04, "OtherAssigned(4)"),
            (0x7F, "OtherAssigned(127)"),
            (0x80, "Private(128)"),
            (0xFE, "Private(254)"),
            (0xFF, "NoAcceptable"),
        ] {
            let method = SocksAuthMethod::from(code);
            assert_eq!(method.code(), code);
            assert_eq!(format!("{}", method), display);
        }
    }
}
