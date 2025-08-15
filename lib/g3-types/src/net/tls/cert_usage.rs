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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_str() {
        assert_eq!(TlsCertUsage::TlsServer.as_str(), "tls_server");
        assert_eq!(
            TlsCertUsage::TLsServerTongsuo.as_str(),
            "tls_server_tongsuo"
        );
        assert_eq!(
            TlsCertUsage::TlcpServerSignature.as_str(),
            "tlcp_server_signature"
        );
        assert_eq!(
            TlsCertUsage::TlcpServerEncryption.as_str(),
            "tlcp_server_encryption"
        );
    }

    #[test]
    fn display() {
        assert_eq!(format!("{}", TlsCertUsage::TlsServer), "tls_server");
        assert_eq!(
            format!("{}", TlsCertUsage::TLsServerTongsuo),
            "tls_server_tongsuo"
        );
        assert_eq!(
            format!("{}", TlsCertUsage::TlcpServerSignature),
            "tlcp_server_signature"
        );
        assert_eq!(
            format!("{}", TlsCertUsage::TlcpServerEncryption),
            "tlcp_server_encryption"
        );
    }

    #[test]
    fn try_from_valid() {
        assert!(matches!(
            TlsCertUsage::try_from(0),
            Ok(TlsCertUsage::TlsServer)
        ));
        assert!(matches!(
            TlsCertUsage::try_from(1),
            Ok(TlsCertUsage::TLsServerTongsuo)
        ));
        assert!(matches!(
            TlsCertUsage::try_from(11),
            Ok(TlsCertUsage::TlcpServerSignature)
        ));
        assert!(matches!(
            TlsCertUsage::try_from(12),
            Ok(TlsCertUsage::TlcpServerEncryption)
        ));
    }

    #[test]
    fn try_from_invalid() {
        assert!(TlsCertUsage::try_from(2).is_err());
        assert!(TlsCertUsage::try_from(10).is_err());
        assert!(TlsCertUsage::try_from(13).is_err());
        assert!(TlsCertUsage::try_from(255).is_err());
    }

    #[test]
    fn from_str_valid() {
        // TlsServer variants
        assert!(matches!(
            TlsCertUsage::from_str("tls_server"),
            Ok(TlsCertUsage::TlsServer)
        ));
        assert!(matches!(
            TlsCertUsage::from_str("TLS_SERVER"),
            Ok(TlsCertUsage::TlsServer)
        ));
        assert!(matches!(
            TlsCertUsage::from_str("tlsserver"),
            Ok(TlsCertUsage::TlsServer)
        ));

        // TLsServerTongsuo variants
        assert!(matches!(
            TlsCertUsage::from_str("tls_server_tongsuo"),
            Ok(TlsCertUsage::TLsServerTongsuo)
        ));
        assert!(matches!(
            TlsCertUsage::from_str("TLS_SERVER_TONGSUO"),
            Ok(TlsCertUsage::TLsServerTongsuo)
        ));
        assert!(matches!(
            TlsCertUsage::from_str("tlsservertongsuo"),
            Ok(TlsCertUsage::TLsServerTongsuo)
        ));

        // TlcpServerSignature variants
        assert!(matches!(
            TlsCertUsage::from_str("tlcp_server_signature"),
            Ok(TlsCertUsage::TlcpServerSignature)
        ));
        assert!(matches!(
            TlsCertUsage::from_str("TLCP_SERVER_SIGNATURE"),
            Ok(TlsCertUsage::TlcpServerSignature)
        ));
        assert!(matches!(
            TlsCertUsage::from_str("tlcp_server_sign"),
            Ok(TlsCertUsage::TlcpServerSignature)
        ));
        assert!(matches!(
            TlsCertUsage::from_str("tlcpserversignature"),
            Ok(TlsCertUsage::TlcpServerSignature)
        ));
        assert!(matches!(
            TlsCertUsage::from_str("tlcpserversign"),
            Ok(TlsCertUsage::TlcpServerSignature)
        ));

        // TlcpServerEncryption variants
        assert!(matches!(
            TlsCertUsage::from_str("tlcp_server_encryption"),
            Ok(TlsCertUsage::TlcpServerEncryption)
        ));
        assert!(matches!(
            TlsCertUsage::from_str("TLCP_SERVER_ENCRYPTION"),
            Ok(TlsCertUsage::TlcpServerEncryption)
        ));
        assert!(matches!(
            TlsCertUsage::from_str("tlcp_server_enc"),
            Ok(TlsCertUsage::TlcpServerEncryption)
        ));
        assert!(matches!(
            TlsCertUsage::from_str("tlcpserverencryption"),
            Ok(TlsCertUsage::TlcpServerEncryption)
        ));
        assert!(matches!(
            TlsCertUsage::from_str("tlcpserverenc"),
            Ok(TlsCertUsage::TlcpServerEncryption)
        ));
    }

    #[test]
    fn from_str_invalid() {
        assert!(TlsCertUsage::from_str("").is_err());
        assert!(TlsCertUsage::from_str("tls_serv").is_err());
        assert!(TlsCertUsage::from_str("server").is_err());
        assert!(TlsCertUsage::from_str("tlcp_sign").is_err());
        assert!(TlsCertUsage::from_str("invalid_usage").is_err());
    }

    #[test]
    fn error_display() {
        let err = InvalidCertUsage;
        assert_eq!(format!("{}", err), "unsupported tls certificate usage type");
    }
}
