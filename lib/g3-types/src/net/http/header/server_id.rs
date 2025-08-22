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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_server_id_operations() {
        // basic accessors
        let server_id = HttpServerId("test-server".to_string());
        assert_eq!(server_id.as_str(), "test-server");
        assert_eq!(server_id.as_bytes(), b"test-server");

        // conversion to header value
        let header_value = server_id.to_header_value();
        assert_eq!(header_value.to_str(), "test-server");

        // TryFrom<String> with valid input
        let valid_string = "valid-server-id".to_string();
        let result = HttpServerId::try_from(valid_string.clone()).unwrap();
        assert_eq!(result.as_str(), "valid-server-id");

        // TryFrom<String> with various invalid characters
        let test_cases = vec![
            ("server\x01id", "position 6", "\\u{1}"), // Control character
            ("server\x7Fid", "position 6", "\\u{7f}"), // DEL character
            ("server;id", "position 6", ";"),         // Semicolon
            ("server,id", "position 6", ","),         // Comma
            ("server café", "position 10", "\\u{e9}"), // Non-ASCII character
        ];

        for (input, expected_pos, expected_char) in test_cases {
            let result = HttpServerId::try_from(input.to_string());
            let err_msg = format!("{}", result.unwrap_err());
            assert!(err_msg.contains(expected_pos));
            assert!(err_msg.contains(expected_char));
        }

        // FromStr with valid and invalid input
        let valid_result = "valid-server".parse::<HttpServerId>().unwrap();
        assert_eq!(valid_result.as_str(), "valid-server");

        let invalid_result = "invalid\x1Fserver".parse::<HttpServerId>();
        let err_msg = format!("{}", invalid_result.unwrap_err());
        assert!(err_msg.contains("position 7"));
        assert!(err_msg.contains("\\u{1f}"));
    }
}
