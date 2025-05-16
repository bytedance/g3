/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;
use std::num::TryFromIntError;
use std::str::FromStr;

use anyhow::anyhow;
use thiserror::Error;

mod v1;
mod v2;

use v1::ProxyProtocolV1Encoder;
pub use v2::ProxyProtocolV2Encoder;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProxyProtocolVersion {
    V1,
    V2,
}

impl FromStr for ProxyProtocolVersion {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "1" | "v1" | "V1" => Ok(ProxyProtocolVersion::V1),
            "2" | "v2" | "V2" => Ok(ProxyProtocolVersion::V2),
            _ => Err(anyhow!("invalid proxy protocol version string")),
        }
    }
}

#[derive(Debug, Error)]
pub enum ProxyProtocolEncodeError {
    #[error("address family not match")]
    AddressFamilyNotMatch,
    #[error("invalid u16 length: {0}")]
    InvalidU16Length(TryFromIntError),
    #[error("total length overflow")]
    TotalLengthOverflow,
    #[error("too long value ({1}) for tag {0}")]
    TooLongTagValue(u8, usize),
}

#[allow(clippy::large_enum_variant)]
pub enum ProxyProtocolEncoder {
    V1(ProxyProtocolV1Encoder),
    V2(ProxyProtocolV2Encoder),
}

impl ProxyProtocolEncoder {
    pub fn new(version: ProxyProtocolVersion) -> Self {
        match version {
            ProxyProtocolVersion::V1 => ProxyProtocolEncoder::V1(ProxyProtocolV1Encoder::new()),
            ProxyProtocolVersion::V2 => ProxyProtocolEncoder::V2(ProxyProtocolV2Encoder::new()),
        }
    }

    pub fn encode_tcp(
        &mut self,
        client_addr: SocketAddr,
        server_addr: SocketAddr,
    ) -> Result<&[u8], ProxyProtocolEncodeError> {
        match self {
            ProxyProtocolEncoder::V1(v1) => v1.encode_tcp(client_addr, server_addr),
            ProxyProtocolEncoder::V2(v2) => v2.encode_tcp(client_addr, server_addr),
        }
    }
}
