/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use base64::prelude::*;

use crate::auth::{AuthParseError, Password, Username};

pub struct HttpBasicAuth {
    pub username: Username,
    pub password: Password,
    encoded_value: String,
}

impl HttpBasicAuth {
    pub fn new(username: Username, password: Password) -> Self {
        let us = username.as_original();
        let ps = password.as_original();
        let mut buf = Vec::with_capacity(us.len() + 1 + ps.len());
        buf.extend_from_slice(us.as_bytes());
        buf.push(b':');
        buf.extend_from_slice(ps.as_bytes());

        let encoded_value = BASE64_STANDARD.encode(buf);

        HttpBasicAuth {
            username,
            password,
            encoded_value,
        }
    }

    #[inline]
    pub fn encoded_value(&self) -> &str {
        &self.encoded_value
    }
}

impl FromStr for HttpBasicAuth {
    type Err = AuthParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let encoded_value = s.trim(); // allow more space than spec

        let decoded = BASE64_STANDARD
            .decode(encoded_value)
            .map_err(|_| AuthParseError::InvalidBase64Encoding)?;
        let value =
            std::str::from_utf8(&decoded).map_err(|_| AuthParseError::InvalidUtf8Encoding)?;

        match memchr::memchr(b':', value.as_bytes()) {
            Some(i) => {
                let username = Username::from_original(&value[0..i])
                    .map_err(|_| AuthParseError::InvalidUsername)?;
                let password = Password::from_original(&value[i + 1..])
                    .map_err(|_| AuthParseError::InvalidPassword)?;
                Ok(HttpBasicAuth {
                    username,
                    password,
                    encoded_value: encoded_value.to_string(),
                })
            }
            None => Err(AuthParseError::NoDelimiterFound),
        }
    }
}

impl TryFrom<&HttpBasicAuth> for http::HeaderValue {
    type Error = http::header::InvalidHeaderValue;

    fn try_from(value: &HttpBasicAuth) -> Result<Self, Self::Error> {
        let value = format!("Basic {}", value.encoded_value());
        http::HeaderValue::from_str(&value)
    }
}
