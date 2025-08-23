/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::fmt;

use thiserror::Error;

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
    def_const_code!(AUTHENTICATION_SUCCESSFUL, b'2', b'3', b'5');
    def_const_code!(OK, b'2', b'5', b'0');

    def_const_code!(AUTH_CONTINUE, b'3', b'3', b'4');
    def_const_code!(START_MAIL_INPUT, b'3', b'5', b'4');

    def_const_code!(SERVICE_NOT_AVAILABLE, b'4', b'2', b'1');

    def_const_code!(BAD_SEQUENCE_OF_COMMANDS, b'5', b'0', b'3');
    def_const_code!(AUTHENTICATION_REQUIRED, b'5', b'3', b'0');
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

#[derive(Default)]
pub struct ResponseParser {
    code: ReplyCode,
    multiline: bool,
    line_count: usize,
}

impl ResponseParser {
    pub const MAX_LINE_SIZE: usize = 512;

    pub fn feed_line<'a>(&mut self, line: &'a [u8]) -> Result<&'a [u8], ResponseLineError> {
        let line = line
            .strip_suffix(b"\r\n")
            .ok_or(ResponseLineError::NoTrailingSequence)?;
        if self.code.is_set() {
            self.feed_following_line(line)
        } else {
            self.feed_first_line(line)
        }
    }

    fn feed_first_line<'a>(&mut self, line: &'a [u8]) -> Result<&'a [u8], ResponseLineError> {
        #[cfg(debug_assertions)]
        if let Ok(s) = std::str::from_utf8(line) {
            log::trace!("[IMAP] --< {s}");
        }

        if line.len() < 3 {
            return Err(ResponseLineError::TooShort);
        }

        self.code =
            ReplyCode::new(line[0], line[1], line[2]).ok_or(ResponseLineError::InvalidCode)?;

        if line.len() == 3 {
            self.multiline = false;
            return Ok(&line[3..]);
        }
        match line[3] {
            b' ' => self.multiline = false,
            b'-' => self.multiline = true,
            _ => return Err(ResponseLineError::InvalidDelimiter),
        }
        self.line_count = 1;
        Ok(&line[4..])
    }

    fn feed_following_line<'a>(&mut self, line: &'a [u8]) -> Result<&'a [u8], ResponseLineError> {
        #[cfg(debug_assertions)]
        if let Ok(s) = std::str::from_utf8(line) {
            log::trace!("[IMAP] +-< {s}");
        }

        if !self.multiline {
            return Err(ResponseLineError::Finished);
        }

        if line.len() < 3 {
            return Err(ResponseLineError::TooShort);
        }

        let code =
            ReplyCode::new(line[0], line[1], line[2]).ok_or(ResponseLineError::InvalidCode)?;
        if code != self.code {
            return Err(ResponseLineError::InvalidCode);
        }

        if line.len() == 3 {
            self.multiline = false;
            return Ok(&line[3..]);
        }
        match line[3] {
            b' ' => self.multiline = false,
            b'-' => {}
            _ => return Err(ResponseLineError::InvalidDelimiter),
        }
        self.line_count += 1;
        Ok(&line[4..])
    }

    pub fn is_first_line(&self) -> bool {
        self.line_count == 1
    }

    pub fn finished(&self) -> bool {
        self.code.is_set() && !self.multiline
    }

    #[inline]
    pub fn code(&self) -> ReplyCode {
        self.code
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_line() {
        let line = b"220 foo.com Simple Mail Transfer Service Ready\r\n";
        let mut rsp = crate::response::ResponseParser::default();
        let msg = rsp.feed_line(line).unwrap();
        assert_eq!(rsp.code.as_u16(), 220);
        assert_eq!(msg, b"foo.com Simple Mail Transfer Service Ready");
        assert!(rsp.finished());
    }

    #[test]
    fn simple_multiline() {
        let line1 = b"250-foo.com greets bar.com\r\n";
        let line2 = b"250-8BITMIME\r\n";
        let line3 = b"250 HELP\r\n";
        let mut rsp = crate::response::ResponseParser::default();

        let msg = rsp.feed_line(line1).unwrap();
        assert_eq!(rsp.code.as_u16(), 250);
        assert_eq!(msg, b"foo.com greets bar.com");
        assert!(!rsp.finished());

        let msg = rsp.feed_line(line2).unwrap();
        assert_eq!(msg, b"8BITMIME");
        assert!(!rsp.finished());

        let msg = rsp.feed_line(line3).unwrap();
        assert_eq!(msg, b"HELP");
        assert!(rsp.finished());
    }

    #[test]
    fn invalid_code() {
        let line = "测试啊 foo.com Simple Mail Transfer Service Ready\r\n";
        let mut rsp = crate::response::ResponseParser::default();
        let err = rsp.feed_line(line.as_bytes()).unwrap_err();
        assert_eq!(err, ResponseLineError::InvalidCode);
    }

    #[test]
    fn empty_end() {
        let line1 = b"250-foo.com greets bar.com\r\n";
        let line2 = b"250-8BITMIME\r\n";
        let line3 = b"250 \r\n";
        let mut rsp = crate::response::ResponseParser::default();

        let msg = rsp.feed_line(line1).unwrap();
        assert_eq!(rsp.code.as_u16(), 250);
        assert_eq!(msg, b"foo.com greets bar.com");
        assert!(!rsp.finished());

        let msg = rsp.feed_line(line2).unwrap();
        assert_eq!(msg, b"8BITMIME");
        assert!(!rsp.finished());

        let msg = rsp.feed_line(line3).unwrap();
        assert_eq!(msg, b"");
        assert!(rsp.finished());
    }
}
