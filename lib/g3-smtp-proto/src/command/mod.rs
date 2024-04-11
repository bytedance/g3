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

use std::str;

use thiserror::Error;

use g3_types::net::Host;

use crate::response::ResponseEncoder;

#[derive(Debug, Error)]
pub enum CommandLineError {
    #[error("no trailing sequence")]
    NoTrailingSequence,
    #[error("invalid utf-8 command")]
    InvalidUtf8Command,
    #[error("invalid client domain/address field")]
    InvalidClientHost,
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
}

impl Command {
    pub const MAX_LINE_SIZE: usize = 512;

    pub fn parse_line(line: &[u8]) -> Result<Self, CommandLineError> {
        let line = line
            .strip_suffix(b"\r\n")
            .ok_or(CommandLineError::NoTrailingSequence)?;

        if let Some(p) = memchr::memchr(b' ', line) {
            // commands with params
            let cmd =
                str::from_utf8(&line[0..p]).map_err(|_| CommandLineError::InvalidUtf8Command)?;
            let upper_cmd = cmd.to_uppercase();

            match upper_cmd.as_bytes() {
                b"EHLO" => {
                    let host = hello_parse_host(&line[p..])?;
                    Ok(Command::ExtendHello(host))
                }
                b"HELO" => {
                    let host = hello_parse_host(&line[p..])?;
                    Ok(Command::Hello(host))
                }
                _ => Ok(Command::Unknown(upper_cmd)),
            }
        } else {
            // commands without params
            let cmd = str::from_utf8(line).map_err(|_| CommandLineError::InvalidUtf8Command)?;
            let upper_cmd = cmd.to_uppercase();

            match upper_cmd.as_bytes() {
                b"QUIT" => Ok(Command::QUIT),
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
