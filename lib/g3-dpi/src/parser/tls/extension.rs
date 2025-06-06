/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use thiserror::Error;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ExtensionType {
    ServerName,                          // rfc6066
    MaxFragmentLength,                   // rfc6066
    StatusRequest,                       // rfc6066
    SupportedGroups,                     // rfc8422, rfc7919
    SignatureAlgorithms,                 // rfc8446
    UseSrtp,                             // rfc5764
    Heartbeat,                           // rfc6520
    ApplicationLayerProtocolNegotiation, // rfc7301
    SignedCertificateTimestamp,          // rfc6962
    ClientCertificateType,               // rfc7250
    ServerCertificateType,               // rfc7250
    Padding,                             // rfc7685
    PreSharedKey,                        // rfc8446(TLS1.3)
    EarlyData,                           // rfc8446(TLS1.3)
    SupportedVersions,                   // rfc8446(TLS1.3)
    Cookie,                              // rfc8446(TLS1.3)
    PskKeyExchangeModes,                 // rfc8446(TLS1.3)
    CertificateAuthorities,              // rfc8446(TLS1.3)
    OidFilters,                          // rfc8446(TLS1.3)
    PostHandshakeAuth,                   // rfc8446(TLS1.3)
    SignatureAlgorithmsCert,             // rfc8446(TLS1.3)
    KeyShare,                            // rfc8446(TLS1.3)
    Unknown(u16),
}

impl From<u16> for ExtensionType {
    fn from(value: u16) -> Self {
        match value {
            0 => ExtensionType::ServerName,
            1 => ExtensionType::MaxFragmentLength,
            5 => ExtensionType::StatusRequest,
            10 => ExtensionType::SupportedGroups,
            13 => ExtensionType::SignatureAlgorithms,
            14 => ExtensionType::UseSrtp,
            15 => ExtensionType::Heartbeat,
            16 => ExtensionType::ApplicationLayerProtocolNegotiation,
            18 => ExtensionType::SignedCertificateTimestamp,
            19 => ExtensionType::ClientCertificateType,
            20 => ExtensionType::ServerCertificateType,
            21 => ExtensionType::Padding,
            41 => ExtensionType::PreSharedKey,
            42 => ExtensionType::EarlyData,
            43 => ExtensionType::SupportedVersions,
            44 => ExtensionType::Cookie,
            45 => ExtensionType::PskKeyExchangeModes,
            47 => ExtensionType::CertificateAuthorities,
            48 => ExtensionType::OidFilters,
            49 => ExtensionType::PostHandshakeAuth,
            50 => ExtensionType::SignatureAlgorithmsCert,
            51 => ExtensionType::KeyShare,
            n => ExtensionType::Unknown(n),
        }
    }
}

#[derive(Debug, Error)]
pub enum ExtensionParseError {
    #[error("not enough data")]
    NotEnoughData,
    #[error("invalid length")]
    InvalidLength,
}

pub struct Extension<'a> {
    ext_type: ExtensionType,
    ext_len: u16,
    ext_data: Option<&'a [u8]>,
}

impl<'a> Extension<'a> {
    const HEADER_LEN: usize = 4;

    pub fn r#type(&self) -> ExtensionType {
        self.ext_type
    }

    pub fn data(&self) -> Option<&[u8]> {
        self.ext_data
    }

    fn parse(data: &'a [u8]) -> Result<Self, ExtensionParseError> {
        if data.len() < Self::HEADER_LEN {
            return Err(ExtensionParseError::NotEnoughData);
        }

        let ext_type = u16::from_be_bytes([data[0], data[1]]);
        let ext_len = u16::from_be_bytes([data[2], data[3]]);

        if ext_len == 0 {
            Ok(Extension {
                ext_type: ext_type.into(),
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
                    ext_type: ext_type.into(),
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
            if ext.ext_type == ext_type {
                return Ok(ext.ext_data);
            }
            offset += Extension::HEADER_LEN + ext.ext_len as usize;
        }

        Ok(None)
    }
}

pub struct ExtensionIter<'a> {
    data: &'a [u8],
    offset: usize,
}

impl<'a> ExtensionIter<'a> {
    pub(super) fn new(data: &'a [u8]) -> Self {
        ExtensionIter { data, offset: 0 }
    }
}

impl<'a> Iterator for ExtensionIter<'a> {
    type Item = Result<Extension<'a>, ExtensionParseError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset < self.data.len() {
            match Extension::parse(&self.data[self.offset..]) {
                Ok(ext) => {
                    self.offset += Extension::HEADER_LEN + ext.ext_len as usize;
                    Some(Ok(ext))
                }
                Err(e) => Some(Err(e)),
            }
        } else {
            None
        }
    }
}
