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
