/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::str::{self, FromStr, Utf8Error};

use thiserror::Error;

use g3_types::net::Host;

use crate::response::ResponseEncoder;

mod path;

mod hello;
mod mail;
mod recipient;

pub use mail::MailParam;
pub use recipient::RecipientParam;

#[derive(Debug, Error)]
pub enum CommandLineError {
    #[error("no trailing sequence")]
    NoTrailingSequence,
    #[error("invalid utf-8 command")]
    InvalidUtf8Command(Utf8Error),
    #[error("invalid client domain/address field")]
    InvalidClientHost,
    #[error("invalid parameter for command {0}: {1}")]
    InvalidCommandParam(&'static str, &'static str),
}

impl From<&CommandLineError> for ResponseEncoder {
    fn from(_value: &CommandLineError) -> Self {
        ResponseEncoder::SYNTAX_ERROR
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Command {
    Quit,
    ExtendHello(Host),
    Hello(Host),
    StartTls,
    Auth,
    AuthenticatedTurn,
    Reset,
    NoOperation,
    Mail(MailParam),
    Recipient(RecipientParam),
    Data,
    BinaryData(usize),
    LastBinaryData(usize),
    DataByUrl(String),
    LastDataByUrl(String),
    KnownForward(&'static str),
    Unknown(String),
}

impl Command {
    pub const MAX_LINE_SIZE: usize = 2048;
    pub const MAX_CONTINUE_LINE_SIZE: usize = 12288; // for AUTH continue line

    pub fn parse_line(line: &[u8]) -> Result<Self, CommandLineError> {
        let line = line
            .strip_suffix(b"\r\n")
            .ok_or(CommandLineError::NoTrailingSequence)?;

        #[cfg(debug_assertions)]
        if let Ok(s) = str::from_utf8(line) {
            log::trace!("[SMTP] --> {s}");
        }

        if let Some(p) = memchr::memchr(b' ', line) {
            // commands with params
            let cmd = str::from_utf8(&line[0..p]).map_err(CommandLineError::InvalidUtf8Command)?;
            let upper_cmd = cmd.to_uppercase();

            let left = &line[p + 1..];
            match upper_cmd.as_bytes() {
                b"EHLO" => {
                    let host = hello::parse_host(left)?;
                    Ok(Command::ExtendHello(host))
                }
                b"HELO" => {
                    let host = hello::parse_host(left)?;
                    Ok(Command::Hello(host))
                }
                b"AUTH" => Ok(Command::Auth),
                b"ATRN" => Ok(Command::AuthenticatedTurn),
                b"MAIL" => {
                    let param = MailParam::parse(left)?;
                    Ok(Command::Mail(param))
                }
                b"RCPT" => {
                    let param = RecipientParam::parse(left)?;
                    Ok(Command::Recipient(param))
                }
                b"BDAT" => binary_data_parse_param(left),
                b"BURL" => burl_data_parse_param(left),
                b"VRFY" => Ok(Command::KnownForward("VRFY")),
                b"EXPN" => Ok(Command::KnownForward("EXPN")),
                b"HELP" => Ok(Command::KnownForward("HELP")),
                b"NOOP" => Ok(Command::NoOperation),
                b"ETRN" => Ok(Command::KnownForward("ETRN")),
                _ => Ok(Command::Unknown(upper_cmd)),
            }
        } else {
            // commands without params
            let cmd = str::from_utf8(line).map_err(CommandLineError::InvalidUtf8Command)?;
            let upper_cmd = cmd.to_uppercase();

            match upper_cmd.as_bytes() {
                b"QUIT" => Ok(Command::Quit),
                b"ATRN" => Ok(Command::AuthenticatedTurn),
                b"STARTTLS" => Ok(Command::StartTls),
                b"RSET" => Ok(Command::Reset),
                b"HELP" => Ok(Command::KnownForward("HELP")),
                b"NOOP" => Ok(Command::NoOperation),
                b"DATA" => Ok(Command::Data),
                _ => Ok(Command::Unknown(upper_cmd)),
            }
        }
    }
}

fn binary_data_parse_param(msg: &[u8]) -> Result<Command, CommandLineError> {
    // bdat-cmd   ::= "BDAT" SP chunk-size [ SP end-marker ] CR LF
    // chunk-size ::= 1*DIGIT
    // end-marker ::= "LAST"

    if let Some(p) = memchr::memchr(b' ', msg) {
        let end_marker = &msg[p + 1..];
        if end_marker != b"LAST" {
            return Err(CommandLineError::InvalidCommandParam(
                "BDAT",
                "invalid end marker",
            ));
        }
        let number = str::from_utf8(&msg[..p]).map_err(CommandLineError::InvalidUtf8Command)?;
        let size = usize::from_str(number)
            .map_err(|_| CommandLineError::InvalidCommandParam("BDAT", "invalid chunk size"))?;
        Ok(Command::LastBinaryData(size))
    } else {
        let number = str::from_utf8(msg).map_err(CommandLineError::InvalidUtf8Command)?;
        let size = usize::from_str(number)
            .map_err(|_| CommandLineError::InvalidCommandParam("BDAT", "invalid chunk size"))?;
        Ok(Command::BinaryData(size))
    }
}

fn burl_data_parse_param(msg: &[u8]) -> Result<Command, CommandLineError> {
    // burl-param  = "imap" / ("imap://" authority)
    // ; parameter to BURL EHLO keyword

    // burl-cmd    = "BURL" SP absolute-URI [SP end-marker] CRLF

    // end-marker  = "LAST"

    if let Some(p) = memchr::memchr(b' ', msg) {
        let end_marker = &msg[p + 1..];
        if end_marker != b"LAST" {
            return Err(CommandLineError::InvalidCommandParam(
                "BURL",
                "invalid end marker",
            ));
        }
        let url = str::from_utf8(&msg[..p]).map_err(CommandLineError::InvalidUtf8Command)?;
        Ok(Command::LastDataByUrl(url.to_string()))
    } else {
        let url = str::from_utf8(msg).map_err(CommandLineError::InvalidUtf8Command)?;
        Ok(Command::DataByUrl(url.to_string()))
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn binary_data() {
        let cmd = Command::parse_line(b"BDAT 1000\r\n").unwrap();
        assert_eq!(cmd, Command::BinaryData(1000));

        let cmd = Command::parse_line(b"BDAT 0 LAST\r\n").unwrap();
        assert_eq!(cmd, Command::LastBinaryData(0));
    }
}
