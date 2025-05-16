/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use url::Url;

use crate::auth::{AuthParseError, Password, Username};
use crate::net::HttpHeaderValue;

mod basic;
pub use basic::HttpBasicAuth;

pub enum HttpAuth {
    None,
    Basic(HttpBasicAuth),
}

impl HttpAuth {
    pub fn from_authorization(value: &str) -> Result<Self, AuthParseError> {
        match memchr::memchr(b' ', value.as_bytes()) {
            Some(i) => match value[0..i].to_ascii_lowercase().as_str() {
                "basic" => {
                    let basic = HttpBasicAuth::from_str(&value[i + 1..])?;
                    Ok(HttpAuth::Basic(basic))
                }
                _ => Ok(HttpAuth::None),
            },
            None => Err(AuthParseError::UnsupportedAuthType),
        }
    }
}

impl TryFrom<&HttpHeaderValue> for HttpAuth {
    type Error = AuthParseError;

    fn try_from(value: &HttpHeaderValue) -> Result<Self, Self::Error> {
        let value = std::str::from_utf8(value.as_bytes())
            .map_err(|_| AuthParseError::InvalidUtf8Encoding)?;
        HttpAuth::from_authorization(value)
    }
}

impl TryFrom<&Url> for HttpAuth {
    type Error = AuthParseError;

    fn try_from(url: &Url) -> Result<Self, Self::Error> {
        let u = url.username();
        let auth = if u.is_empty() {
            HttpAuth::None
        } else {
            let username =
                Username::from_encoded(u).map_err(|_| AuthParseError::InvalidUsername)?;

            let password = if let Some(p) = url.password() {
                Password::from_encoded(p).map_err(|_| AuthParseError::InvalidPassword)?
            } else {
                return Err(AuthParseError::InvalidPassword);
            };

            HttpAuth::Basic(HttpBasicAuth::new(username, password))
        };

        Ok(auth)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_ok() -> Result<(), ()> {
        let value = "Basic cm9vdDp0b29y";
        let info = HttpAuth::from_authorization(value).unwrap();
        if let HttpAuth::Basic(HttpBasicAuth {
            username, password, ..
        }) = info
        {
            assert_eq!(username.as_original(), "root");
            assert_eq!(password.as_original(), "toor");
            Ok(())
        } else {
            Err(())
        }
    }

    #[test]
    fn parse_scheme_only() {
        let value = "Basic ";
        let result = HttpAuth::from_authorization(value);
        assert!(result.is_err());
    }
}
