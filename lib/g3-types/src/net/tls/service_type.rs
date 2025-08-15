/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::fmt;
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Hash, Eq, PartialEq)]
#[repr(u8)]
pub enum TlsServiceType {
    Http = 0,
    Smtp = 1,
    Imap = 2,
}

impl TlsServiceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            TlsServiceType::Http => "http",
            TlsServiceType::Smtp => "smtp",
            TlsServiceType::Imap => "imap",
        }
    }
}

impl fmt::Display for TlsServiceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

pub struct InvalidServiceType;

impl fmt::Display for InvalidServiceType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unsupported tls service type")
    }
}

impl TryFrom<u8> for TlsServiceType {
    type Error = InvalidServiceType;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(TlsServiceType::Http),
            1 => Ok(TlsServiceType::Smtp),
            2 => Ok(TlsServiceType::Imap),
            _ => Err(InvalidServiceType),
        }
    }
}

impl FromStr for TlsServiceType {
    type Err = InvalidServiceType;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "http" | "HTTP" => Ok(TlsServiceType::Http),
            "smtp" | "SMTP" => Ok(TlsServiceType::Smtp),
            "imap" | "IMAP" => Ok(TlsServiceType::Imap),
            _ => Err(InvalidServiceType),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn as_str() {
        assert_eq!(TlsServiceType::Http.as_str(), "http");
        assert_eq!(TlsServiceType::Smtp.as_str(), "smtp");
        assert_eq!(TlsServiceType::Imap.as_str(), "imap");
    }

    #[test]
    fn display() {
        assert_eq!(format!("{}", TlsServiceType::Http), "http");
        assert_eq!(format!("{}", TlsServiceType::Smtp), "smtp");
        assert_eq!(format!("{}", TlsServiceType::Imap), "imap");
    }

    #[test]
    fn try_from_u8_valid() {
        assert!(matches!(
            TlsServiceType::try_from(0),
            Ok(TlsServiceType::Http)
        ));
        assert!(matches!(
            TlsServiceType::try_from(1),
            Ok(TlsServiceType::Smtp)
        ));
        assert!(matches!(
            TlsServiceType::try_from(2),
            Ok(TlsServiceType::Imap)
        ));
    }

    #[test]
    fn try_from_u8_invalid() {
        assert!(TlsServiceType::try_from(3).is_err());
        assert!(TlsServiceType::try_from(255).is_err());
    }

    #[test]
    fn from_str_valid() {
        assert!(matches!("http".parse(), Ok(TlsServiceType::Http)));
        assert!(matches!("HTTP".parse(), Ok(TlsServiceType::Http)));
        assert!(matches!("smtp".parse(), Ok(TlsServiceType::Smtp)));
        assert!(matches!("SMTP".parse(), Ok(TlsServiceType::Smtp)));
        assert!(matches!("imap".parse(), Ok(TlsServiceType::Imap)));
        assert!(matches!("IMAP".parse(), Ok(TlsServiceType::Imap)));
    }

    #[test]
    fn from_str_invalid() {
        assert!("https".parse::<TlsServiceType>().is_err());
        assert!("ftp".parse::<TlsServiceType>().is_err());
        assert!("pop3".parse::<TlsServiceType>().is_err());
        assert!("".parse::<TlsServiceType>().is_err());
    }

    #[test]
    fn invalid_service_type_display() {
        let err = InvalidServiceType;
        assert_eq!(format!("{}", err), "unsupported tls service type");
    }
}
