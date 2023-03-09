/*
 * Copyright 2023 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use std::str::FromStr;

use http::HeaderValue;
use url::Url;

use crate::auth::{AuthParseError, Password, Username};

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

impl TryFrom<&HeaderValue> for HttpAuth {
    type Error = AuthParseError;

    fn try_from(value: &HeaderValue) -> Result<Self, Self::Error> {
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
