/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

use thiserror::Error;

use crate::codec::{BerInteger, BerIntegerParseError, LdapSequence, LdapSequenceParseError};

#[derive(Debug, Error)]
pub enum LdapResultParseError {
    #[error("invalid result code value: {0}")]
    InvalidResultCode(#[from] BerIntegerParseError),
    #[error("out of range result code")]
    OutOfRangeResultCode,
    #[error("invalid matched dn string: {0}")]
    InvalidMatchedDn(LdapSequenceParseError),
    #[error("invalid diagnostic message string: {0}")]
    InvalidDiagnosticMessage(LdapSequenceParseError),
    #[error("invalid referrals sequence: {0}")]
    InvalidReferralsSequence(LdapSequenceParseError),
}

pub struct LdapResult<'a> {
    result_code: u16,
    matched_dn: &'a [u8],
    diagnostic_message: &'a [u8],
    #[allow(unused)]
    referral_sequence: &'a [u8], // only if code is REFERRAL 0x0a
    encoded_len: usize,
}

impl<'a> LdapResult<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, LdapResultParseError> {
        let code = BerInteger::parse_enumerated_value(data)?;
        let result_code =
            u16::try_from(code.value()).map_err(|_| LdapResultParseError::OutOfRangeResultCode)?;
        let mut offset = code.encoded_len();

        let matched_dn = LdapSequence::parse_octet_string(&data[offset..])
            .map_err(LdapResultParseError::InvalidMatchedDn)?;
        offset += matched_dn.encoded_len();

        let diagnostic_message = LdapSequence::parse_octet_string(&data[offset..])
            .map_err(LdapResultParseError::InvalidDiagnosticMessage)?;
        offset += diagnostic_message.encoded_len();

        if result_code == 10 {
            let referral_sequence = LdapSequence::parse_referrals_sequence(&data[offset..])
                .map_err(LdapResultParseError::InvalidReferralsSequence)?;
            Ok(LdapResult {
                result_code,
                matched_dn: matched_dn.data(),
                diagnostic_message: diagnostic_message.data(),
                referral_sequence: referral_sequence.data(),
                encoded_len: offset + referral_sequence.encoded_len(),
            })
        } else {
            Ok(LdapResult {
                result_code,
                matched_dn: matched_dn.data(),
                diagnostic_message: diagnostic_message.data(),
                referral_sequence: b"",
                encoded_len: offset,
            })
        }
    }

    #[inline]
    pub fn result_code(&self) -> u16 {
        self.result_code
    }

    #[inline]
    pub fn matched_dn(&self) -> &[u8] {
        self.matched_dn
    }

    #[inline]
    pub fn diagnostic_message(&self) -> &[u8] {
        self.diagnostic_message
    }

    #[inline]
    pub fn encoded_len(&self) -> usize {
        self.encoded_len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_simple() {
        let data = &[
            0x0a, 0x01, 0x00, // result code 0
            0x04, 0x00, // no matched dn
            0x04, 0x00, // no diagnostic message
        ];
        let v = LdapResult::parse(data).unwrap();
        assert_eq!(v.result_code(), 0);
        assert!(v.matched_dn().is_empty());
        assert!(v.diagnostic_message().is_empty());
        assert_eq!(v.encoded_len(), 7);
    }
}
