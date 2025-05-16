/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use thiserror::Error;

#[derive(Clone, Copy)]
#[repr(u16)]
pub enum ExtensionType {
    ServerName = 0,                           // rfc6066
    MaxFragmentLength = 1,                    // rfc6066
    StatusRequest = 5,                        // rfc6066
    SupportedGroups = 10,                     // rfc8422, rfc7919
    SignatureAlgorithms = 13,                 // rfc8446
    UseSrtp = 14,                             // rfc5764
    Heartbeat = 15,                           // rfc6520
    ApplicationLayerProtocolNegotiation = 16, // rfc7301
    SignedCertificateTimestamp = 18,          // rfc6962
    ClientCertificateType = 19,               // rfc7250
    ServerCertificateType = 20,               // rfc7250
    Padding = 21,                             // rfc7685
    PreSharedKey = 41,                        // rfc8446(TLS1.3)
    EarlyData = 42,                           // rfc8446(TLS1.3)
    SupportedVersions = 43,                   // rfc8446(TLS1.3)
    Cookie = 44,                              // rfc8446(TLS1.3)
    PskKeyExchangeModes = 45,                 // rfc8446(TLS1.3)
    CertificateAuthorities = 47,              // rfc8446(TLS1.3)
    OidFilters = 48,                          // rfc8446(TLS1.3)
    PostHandshakeAuth = 49,                   // rfc8446(TLS1.3)
    SignatureAlgorithmsCert = 50,             // rfc8446(TLS1.3)
    KeyShare = 51,                            // rfc8446(TLS1.3)
}

#[derive(Debug, Error)]
pub enum ExtensionParseError {
    #[error("not enough data")]
    NotEnoughData,
    #[error("invalid length")]
    InvalidLength,
}

struct Extension<'a> {
    ext_type: u16,
    ext_len: u16,
    ext_data: Option<&'a [u8]>,
}

impl<'a> Extension<'a> {
    const HEADER_LEN: usize = 4;

    fn parse(data: &'a [u8]) -> Result<Self, ExtensionParseError> {
        if data.len() < Self::HEADER_LEN {
            return Err(ExtensionParseError::NotEnoughData);
        }

        let ext_type = u16::from_be_bytes([data[0], data[1]]);
        let ext_len = u16::from_be_bytes([data[2], data[3]]);

        if ext_len == 0 {
            Ok(Extension {
                ext_type,
                ext_len,
                ext_data: None,
            })
        } else {
            let start = Self::HEADER_LEN;
            let end = start + ext_len as usize;
            if end > data.len() {
                Err(ExtensionParseError::InvalidLength)
            } else {
                Ok(Extension {
                    ext_type,
                    ext_len,
                    ext_data: Some(&data[start..end]),
                })
            }
        }
    }
}

pub struct ExtensionList {}

impl ExtensionList {
    /// Get the raw extension value from the raw extensions buf
    pub(crate) fn get_ext(
        full_data: &[u8],
        ext_type: ExtensionType,
    ) -> Result<Option<&[u8]>, ExtensionParseError> {
        let mut offset = 0usize;

        while offset < full_data.len() {
            let left = &full_data[offset..];
            let ext = Extension::parse(left)?;
            if ext.ext_type == ext_type as u16 {
                return Ok(ext.ext_data);
            }
            offset += Extension::HEADER_LEN + ext.ext_len as usize;
        }

        Ok(None)
    }
}
