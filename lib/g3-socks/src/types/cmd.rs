/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::fmt;

use super::SocksNegotiationError;

#[derive(Debug)]
pub enum SocksCommand {
    TcpConnect = 0x01,
    TcpBind = 0x02,
    UdpAssociate = 0x03,
}

impl SocksCommand {
    pub const fn code(&self) -> u8 {
        match self {
            SocksCommand::TcpConnect => 0x01,
            SocksCommand::TcpBind => 0x02,
            SocksCommand::UdpAssociate => 0x03,
        }
    }
}

impl fmt::Display for SocksCommand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SocksCommand::TcpConnect => f.write_str("TcpConnect"),
            SocksCommand::TcpBind => f.write_str("TcpBind"),
            SocksCommand::UdpAssociate => f.write_str("UdpAssociate"),
        }
    }
}

impl TryFrom<u8> for SocksCommand {
    type Error = SocksNegotiationError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0x01 => Ok(SocksCommand::TcpConnect),
            0x02 => Ok(SocksCommand::TcpBind),
            0x03 => Ok(SocksCommand::UdpAssociate),
            _ => Err(SocksNegotiationError::InvalidCommand),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn operations() {
        for (code, display) in [
            (0x01, "TcpConnect"),
            (0x02, "TcpBind"),
            (0x03, "UdpAssociate"),
        ] {
            let cmd = SocksCommand::try_from(code).unwrap();
            assert_eq!(cmd.code(), code);
            assert_eq!(format!("{}", cmd), display);
        }

        for code in [0x00, 0x04, 0xFF] {
            assert!(matches!(
                SocksCommand::try_from(code).unwrap_err(),
                SocksNegotiationError::InvalidCommand
            ));
        }
    }
}
