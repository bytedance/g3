/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use crate::error::FoundInvalidChar;
use crate::net::HttpHeaderValue;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpServerId(String);

impl HttpServerId {
    #[inline]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    #[inline]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    pub fn to_header_value(&self) -> HttpHeaderValue {
        unsafe { HttpHeaderValue::from_string_unchecked(self.0.clone()) }
    }
}

impl TryFrom<String> for HttpServerId {
    type Error = FoundInvalidChar;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        check_invalid_chars(value.as_str())?;
        Ok(HttpServerId(value))
    }
}

impl FromStr for HttpServerId {
    type Err = FoundInvalidChar;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        check_invalid_chars(s)?;
        Ok(HttpServerId(s.to_string()))
    }
}

fn check_invalid_chars(s: &str) -> Result<(), FoundInvalidChar> {
    for (i, c) in s.chars().enumerate() {
        if c.is_ascii() {
            if matches!(c, '\0'..='\x1F' | '\x7F' | ';' | ',') {
                return Err(FoundInvalidChar::new(i, c));
            }
        } else {
            return Err(FoundInvalidChar::new(i, c));
        }
    }
    Ok(())
}
