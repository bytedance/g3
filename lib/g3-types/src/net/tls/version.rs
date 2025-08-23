/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::fmt;
use std::str::FromStr;

use anyhow::anyhow;
#[cfg(feature = "openssl")]
use openssl::ssl::SslVersion;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TlsVersion {
    TLS1_0,
    TLS1_1,
    TLS1_2,
    TLS1_3,
}

impl TlsVersion {
    pub fn as_str(&self) -> &'static str {
        match self {
            TlsVersion::TLS1_0 => "TLS1.0",
            TlsVersion::TLS1_1 => "TLS1.1",
            TlsVersion::TLS1_2 => "TLS1.2",
            TlsVersion::TLS1_3 => "TLS1.3",
        }
    }
}

impl FromStr for TlsVersion {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "1.0" | "tls10" | "tls1.0" | "tls1_0" => Ok(TlsVersion::TLS1_0),
            "1.1" | "tls11" | "tls1.1" | "tls1_1" => Ok(TlsVersion::TLS1_1),
            "1.2" | "tls12" | "tls1.2" | "tls1_2" => Ok(TlsVersion::TLS1_2),
            "1.3" | "tls13" | "tls1.3" | "tls1_3" => Ok(TlsVersion::TLS1_3),
            _ => Err(anyhow!("unknown TLS version {s}")),
        }
    }
}

impl TryFrom<f64> for TlsVersion {
    type Error = anyhow::Error;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        match value {
            1.0 => Ok(TlsVersion::TLS1_0),
            1.1 => Ok(TlsVersion::TLS1_1),
            1.2 => Ok(TlsVersion::TLS1_2),
            1.3 => Ok(TlsVersion::TLS1_3),
            _ => Err(anyhow!("unknown TLS version {value}")),
        }
    }
}

#[cfg(feature = "openssl")]
impl From<TlsVersion> for SslVersion {
    fn from(value: TlsVersion) -> Self {
        match value {
            TlsVersion::TLS1_0 => SslVersion::TLS1,
            TlsVersion::TLS1_1 => SslVersion::TLS1_1,
            TlsVersion::TLS1_2 => SslVersion::TLS1_2,
            TlsVersion::TLS1_3 => SslVersion::TLS1_3,
        }
    }
}

impl fmt::Display for TlsVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_str() {
        assert_eq!(TlsVersion::TLS1_0.as_str(), "TLS1.0");
        assert_eq!(TlsVersion::TLS1_1.as_str(), "TLS1.1");
        assert_eq!(TlsVersion::TLS1_2.as_str(), "TLS1.2");
        assert_eq!(TlsVersion::TLS1_3.as_str(), "TLS1.3");
    }

    #[test]
    fn from_str_valid() {
        assert_eq!(TlsVersion::from_str("1.0").unwrap(), TlsVersion::TLS1_0);
        assert_eq!(TlsVersion::from_str("tls10").unwrap(), TlsVersion::TLS1_0);
        assert_eq!(TlsVersion::from_str("tls1.0").unwrap(), TlsVersion::TLS1_0);
        assert_eq!(TlsVersion::from_str("tls1_0").unwrap(), TlsVersion::TLS1_0);

        assert_eq!(TlsVersion::from_str("1.1").unwrap(), TlsVersion::TLS1_1);
        assert_eq!(TlsVersion::from_str("tls11").unwrap(), TlsVersion::TLS1_1);
        assert_eq!(TlsVersion::from_str("tls1.1").unwrap(), TlsVersion::TLS1_1);
        assert_eq!(TlsVersion::from_str("tls1_1").unwrap(), TlsVersion::TLS1_1);

        assert_eq!(TlsVersion::from_str("1.2").unwrap(), TlsVersion::TLS1_2);
        assert_eq!(TlsVersion::from_str("tls12").unwrap(), TlsVersion::TLS1_2);
        assert_eq!(TlsVersion::from_str("tls1.2").unwrap(), TlsVersion::TLS1_2);
        assert_eq!(TlsVersion::from_str("tls1_2").unwrap(), TlsVersion::TLS1_2);

        assert_eq!(TlsVersion::from_str("1.3").unwrap(), TlsVersion::TLS1_3);
        assert_eq!(TlsVersion::from_str("tls13").unwrap(), TlsVersion::TLS1_3);
        assert_eq!(TlsVersion::from_str("tls1.3").unwrap(), TlsVersion::TLS1_3);
        assert_eq!(TlsVersion::from_str("tls1_3").unwrap(), TlsVersion::TLS1_3);
    }

    #[test]
    fn from_str_invalid() {
        assert!(TlsVersion::from_str("").is_err());
        assert!(TlsVersion::from_str("TLS2.0").is_err());
        assert!(TlsVersion::from_str("ssl3.0").is_err());
        assert!(TlsVersion::from_str("tls").is_err());
        assert!(TlsVersion::from_str("1.4").is_err());
        assert!(TlsVersion::from_str("tls14").is_err());
    }

    #[test]
    fn try_from_f64() {
        assert_eq!(TlsVersion::try_from(1.0).unwrap(), TlsVersion::TLS1_0);
        assert_eq!(TlsVersion::try_from(1.1).unwrap(), TlsVersion::TLS1_1);
        assert_eq!(TlsVersion::try_from(1.2).unwrap(), TlsVersion::TLS1_2);
        assert_eq!(TlsVersion::try_from(1.3).unwrap(), TlsVersion::TLS1_3);

        assert!(TlsVersion::try_from(0.9).is_err());
        assert!(TlsVersion::try_from(1.5).is_err());
        assert!(TlsVersion::try_from(2.0).is_err());
        assert!(TlsVersion::try_from(-1.0).is_err());
    }

    #[test]
    fn display() {
        assert_eq!(format!("{}", TlsVersion::TLS1_0), "TLS1.0");
        assert_eq!(format!("{}", TlsVersion::TLS1_1), "TLS1.1");
        assert_eq!(format!("{}", TlsVersion::TLS1_2), "TLS1.2");
        assert_eq!(format!("{}", TlsVersion::TLS1_3), "TLS1.3");
    }

    #[cfg(feature = "openssl")]
    mod openssl_tests {
        use super::*;

        #[test]
        fn into_ssl_version() {
            assert_eq!(SslVersion::from(TlsVersion::TLS1_0), SslVersion::TLS1);
            assert_eq!(SslVersion::from(TlsVersion::TLS1_1), SslVersion::TLS1_1);
            assert_eq!(SslVersion::from(TlsVersion::TLS1_2), SslVersion::TLS1_2);
            assert_eq!(SslVersion::from(TlsVersion::TLS1_3), SslVersion::TLS1_3);
        }
    }
}
