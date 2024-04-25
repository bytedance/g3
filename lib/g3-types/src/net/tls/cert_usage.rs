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

use std::fmt;
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
#[repr(u8)]
pub enum TlsCertUsage {
    TlsServer = 0,
    TLsServerTongsuo = 1,
    TlcpServerSignature = 11,
    TlcpServerEncryption = 12,
}

impl TlsCertUsage {
    pub fn as_str(&self) -> &'static str {
        match self {
            TlsCertUsage::TlsServer => "tls_server",
            TlsCertUsage::TLsServerTongsuo => "tls_server_tongsuo",
            TlsCertUsage::TlcpServerSignature => "tlcp_server_signature",
            TlsCertUsage::TlcpServerEncryption => "tlcp_server_encryption",
        }
    }
}

impl fmt::Display for TlsCertUsage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

pub struct InvalidCertUsage;

impl fmt::Display for InvalidCertUsage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unsupported tls certificate usage type")
    }
}

impl TryFrom<u8> for TlsCertUsage {
    type Error = InvalidCertUsage;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(TlsCertUsage::TlsServer),
            1 => Ok(TlsCertUsage::TLsServerTongsuo),
            11 => Ok(TlsCertUsage::TlcpServerSignature),
            12 => Ok(TlsCertUsage::TlcpServerEncryption),
            _ => Err(InvalidCertUsage),
        }
    }
}

impl FromStr for TlsCertUsage {
    type Err = InvalidCertUsage;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "tls_server" | "tlsserver" => Ok(TlsCertUsage::TlsServer),
            "tls_server_tongsuo" | "tlsservertongsuo" => Ok(TlsCertUsage::TLsServerTongsuo),
            "tlcp_server_signature"
            | "tlcp_server_sign"
            | "tlcpserversignature"
            | "tlcpserversign" => Ok(TlsCertUsage::TlcpServerSignature),
            "tlcp_server_encryption"
            | "tlcp_server_enc"
            | "tlcpserverencryption"
            | "tlcpserverenc" => Ok(TlsCertUsage::TlcpServerEncryption),
            _ => Err(InvalidCertUsage),
        }
    }
}
