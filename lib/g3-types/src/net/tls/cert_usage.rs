/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
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
