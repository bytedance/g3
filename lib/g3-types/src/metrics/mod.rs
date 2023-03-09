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

use thiserror::Error;

mod name;
pub use name::MetricsName;

mod tag;
pub use tag::{MetricsTagName, MetricsTagValue, StaticMetricsTags};

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("invalid graphic char: {0}")]
    InvalidGraphic(char),
    #[error("not alpha numeric char")]
    NotAlphaNumeric,
}

fn chars_allowed_in_opentsdb(s: &str) -> Result<(), ParseError> {
    for c in s.chars() {
        // Same character range as OpenTSDB
        // http://opentsdb.net/docs/build/html/user_guide/writing/index.html#metrics-and-tags
        if c.is_ascii() {
            match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '/' => {}
                _ => {
                    return if c.is_ascii_graphic() {
                        Err(ParseError::InvalidGraphic(c))
                    } else {
                        Err(ParseError::NotAlphaNumeric)
                    };
                }
            }
        } else if !c.is_alphanumeric() {
            return Err(ParseError::NotAlphaNumeric);
        }
    }
    Ok(())
}
