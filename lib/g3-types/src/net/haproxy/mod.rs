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

use std::net::SocketAddr;
use std::str::FromStr;

use anyhow::anyhow;
use thiserror::Error;

mod v1;
mod v2;

use v1::ProxyProtocolV1Encoder;
use v2::ProxyProtocolV2Encoder;

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
}

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
