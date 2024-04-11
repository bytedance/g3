/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use std::fmt;

use thiserror::Error;

mod parser;
pub use parser::ResponseParser;

mod encoder;
pub use encoder::ResponseEncoder;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ResponseLineError {
    #[error("no trailing sequence")]
    NoTrailingSequence,
    #[error("too short")]
    TooShort,
    #[error("invalid code")]
    InvalidCode,
    #[error("invalid delimiter")]
    InvalidDelimiter,
    #[error("finished")]
    Finished,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReplyCode {
    a: u8,
    b: u8,
    c: u8,
}

macro_rules! def_const_code {
    ($name:ident, $a:literal, $b:literal, $c:literal) => {
        pub const $name: ReplyCode = ReplyCode {
            a: $a,
            b: $b,
            c: $c,
        };
    };
}

impl ReplyCode {
    def_const_code!(SERVICE_READY, b'2', b'2', b'0');
    def_const_code!(SERVICE_CLOSING, b'2', b'2', b'1');

    def_const_code!(BAD_SEQUENCE_OF_COMMANDS, b'5', b'0', b'3');
    def_const_code!(NO_SERVICE, b'5', b'5', b'4');

    fn new(a: u8, b: u8, c: u8) -> Option<Self> {
        if !(0x32u8..=0x35u8).contains(&a) {
            return None;
        }
        if !(0x30..=0x35).contains(&b) {
            return None;
        }
        if !(0x30..=0x39).contains(&c) {
            return None;
        }
        Some(ReplyCode { a, b, c })
    }

    fn is_set(&self) -> bool {
        self.a != 0
    }

    pub fn as_u16(&self) -> u16 {
        (self.a - b'0') as u16 * 100 + (self.b - b'0') as u16 * 10 + (self.c - b'0') as u16
    }
}

impl fmt::Display for ReplyCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}{}{}", self.a as char, self.b as char, self.c as char)
    }
}
