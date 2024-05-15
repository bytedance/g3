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

use std::str::{self, FromStr, Utf8Error};

use thiserror::Error;

use g3_types::net::Host;

use crate::response::ResponseEncoder;

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
    QUIT,
    ExtendHello(Host),
    Hello(Host),
    Unknown(String),
    Data,
    BinaryData(usize),
    LastBinaryData(usize),
    ETRN,
}

impl Command {
    pub const MAX_LINE_SIZE: usize = 512;

    pub fn parse_line(line: &[u8]) -> Result<Self, CommandLineError> {
        let line = line
            .strip_suffix(b"\r\n")
            .ok_or(CommandLineError::NoTrailingSequence)?;

        if let Some(p) = memchr::memchr(b' ', line) {
            // commands with params
            let cmd = str::from_utf8(&line[0..p]).map_err(CommandLineError::InvalidUtf8Command)?;
            let upper_cmd = cmd.to_uppercase();

            let left = &line[p + 1..];
            match upper_cmd.as_bytes() {
                b"EHLO" => {
                    let host = hello_parse_host(left)?;
                    Ok(Command::ExtendHello(host))
                }
                b"HELO" => {
                    let host = hello_parse_host(left)?;
                    Ok(Command::Hello(host))
                }
                b"BDAT" => binary_data_parse_param(left),
                b"ETRN" => Ok(Command::ETRN),
                _ => Ok(Command::Unknown(upper_cmd)),
            }
        } else {
            // commands without params
            let cmd = str::from_utf8(line).map_err(CommandLineError::InvalidUtf8Command)?;
            let upper_cmd = cmd.to_uppercase();

            match upper_cmd.as_bytes() {
                b"QUIT" => Ok(Command::QUIT),
                b"DATA" => Ok(Command::Data),
                _ => Ok(Command::Unknown(upper_cmd)),
            }
        }
    }
}

fn hello_parse_host(msg: &[u8]) -> Result<Host, CommandLineError> {
    let host_b = match memchr::memchr(b' ', msg) {
        Some(p) => &msg[..p],
        None => msg,
    };
    Host::parse_smtp_host_address(host_b).ok_or(CommandLineError::InvalidClientHost)
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
