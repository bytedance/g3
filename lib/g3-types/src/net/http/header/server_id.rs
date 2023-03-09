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

use crate::error::FoundInvalidChar;

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

    pub fn to_header_value(&self) -> HeaderValue {
        unsafe { HeaderValue::from_maybe_shared_unchecked(self.0.clone()) }
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
