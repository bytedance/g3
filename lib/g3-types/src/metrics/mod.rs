/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::Utf8Error;

use thiserror::Error;

mod name;
pub use name::NodeName;

mod tag;
pub use tag::{MetricTagMap, MetricTagName, MetricTagValue};

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("unexpected empty")]
    UnexpectedEmpty,
    #[error("invalid graphic char: {0}")]
    InvalidGraphic(char),
    #[error("not alpha numeric char")]
    NotAlphaNumeric,
    #[error("not valid utf-8 string: {0}")]
    NotValidUtf8(#[from] Utf8Error),
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
