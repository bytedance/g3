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
            SocksCommand::TcpConnect => write!(f, "TcpConnect"),
            SocksCommand::TcpBind => write!(f, "TcpBind"),
            SocksCommand::UdpAssociate => write!(f, "UdpAssociate"),
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
