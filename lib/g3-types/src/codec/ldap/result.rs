/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

pub enum LdapResultParseError {
    NotEnoughData,
    InvalidBerType,
}

pub struct LdapResult<'a> {
    result_code: u16,
    matched_dn: &'a [u8],
    diagnostic_message: &'a [u8],
    referral: &'a [u8], // only if code is REFERRAL 0x0a
}

impl<'a> LdapResult<'a> {
    pub fn parse(data: &'a [u8]) -> Result<Self, LdapResultParseError> {
        if data.is_empty() {
            return Err(LdapResultParseError::NotEnoughData);
        }
        if data[0] != 0x0a {
            return Err(LdapResultParseError::InvalidBerType);
        }

        // TODO parse code as integer

        todo!()
    }
}
