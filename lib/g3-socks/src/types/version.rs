/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

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
            SocksVersion::V4a => f.write_str("socks v4(a)"),
            SocksVersion::V5 => f.write_str("socks v5"),
            SocksVersion::V6 => f.write_str("socks v6"),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operations() {
        for (code, display) in [
            (0x04, "socks v4(a)"),
            (0x05, "socks v5"),
            (0x06, "socks v6"),
        ] {
            let v = SocksVersion::try_from(code).unwrap();
            assert_eq!(v.code(), code);
            assert_eq!(format!("{}", v), display);
        }

        for code in [0x00, 0x03, 0x07, 0xFF] {
            assert!(matches!(
                SocksVersion::try_from(code).unwrap_err(),
                SocksNegotiationError::InvalidVersion
            ));
        }
    }
}
