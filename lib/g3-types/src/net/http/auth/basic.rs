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
