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
use std::str::{self, Utf8Error};

use atoi::FromRadix10Checked;
use log::trace;
use smol_str::SmolStr;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CommandLineError {
    #[error("no trailing sequence")]
    NoTrailingSequence,
    #[error("no tag found as a prefix")]
    NotTagPrefixed,
    #[error("invalid utf-8 command: {0}")]
    InvalidUtf8Command(Utf8Error),
    #[error("invalid literal format")]
    InvalidLiteralFormat,
    #[error("invalid literal size")]
    InvalidLiteralSize,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParsedCommand {
    Capability,
    NoOperation,
    Logout,
    StartTls,
    Auth,
    Login,
    Enable,
    Select,
    Examine,
    Create,
    Delete,
    Rename,
    Subscribe,
    Unsubscribe,
    List,
    Lsub,
    Namespace,
    Status,
    Append,
    Idle,
    Close,
    Unselect,
    Expunge,
    Search,
    Fetch,
    Store,
    Copy,
    Move,
    Uid,
    Id,
    Unknown,
}

impl ParsedCommand {
    pub fn require_untagged_response(&self) -> bool {
        matches!(
            self,
            ParsedCommand::Capability
                | ParsedCommand::NoOperation
                | ParsedCommand::Select
                | ParsedCommand::Examine
                | ParsedCommand::List
                | ParsedCommand::Namespace
                | ParsedCommand::Status
        )
    }
}

#[derive(Clone, Copy)]
pub struct LiteralArgument {
    pub size: usize,
    pub wait_continuation: bool,
}

impl LiteralArgument {
    fn parse_size(buf: &[u8]) -> Result<Self, CommandLineError> {
        if buf.is_empty() {
            return Err(CommandLineError::InvalidLiteralFormat);
        }
        let (size, offset) = usize::from_radix_10_checked(buf);
        let Some(size) = size else {
            return Err(CommandLineError::InvalidLiteralSize);
        };
        if offset == 0 {
            return Err(CommandLineError::InvalidLiteralFormat);
        } else if offset == buf.len() {
            return Ok(LiteralArgument {
                size,
                wait_continuation: true,
            });
        } else if offset + 1 == buf.len() && buf[offset] == b'+' {
            if size > 4096 {
                return Err(CommandLineError::InvalidLiteralSize);
            }
            return Ok(LiteralArgument {
                size,
                wait_continuation: false,
            });
        }

        Err(CommandLineError::InvalidLiteralFormat)
    }

    fn check(left: &[u8]) -> Result<Option<Self>, CommandLineError> {
        if left.ends_with(b"}") {
            if let Some(p) = memchr::memrchr(b'{', left) {
                let arg = Self::parse_size(&left[p + 1..left.len() - 1])?;
                return Ok(Some(arg));
            }
        }
        Ok(None)
    }
}

pub struct Command {
    pub tag: SmolStr,
    pub parsed: ParsedCommand,
    pub literal_arg: Option<LiteralArgument>,
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}/{}", self.parsed, self.tag)
    }
}

impl Command {
    pub fn parse_line(line: &[u8]) -> Result<Self, CommandLineError> {
        let left = line
            .strip_suffix(b"\r\n")
            .ok_or(CommandLineError::NoTrailingSequence)?;

        #[cfg(debug_assertions)]
        if let Ok(s) = str::from_utf8(left) {
            trace!("[IMAP] --> {s}");
        }

        let Some(d) = memchr::memchr(b' ', left) else {
            return Err(CommandLineError::NotTagPrefixed);
        };

        let tag = str::from_utf8(&left[..d]).map_err(CommandLineError::InvalidUtf8Command)?;
        let left = &left[d + 1..];
        if left.is_empty() {
            return Err(CommandLineError::NotTagPrefixed);
        }

        if let Some(p) = memchr::memchr(b' ', left) {
            // commands with params
            let cmd = str::from_utf8(&left[0..p]).map_err(CommandLineError::InvalidUtf8Command)?;
            let upper_cmd = cmd.to_uppercase();

            let left = &left[p + 1..];
            let literal_arg = LiteralArgument::check(left)?;
            let parsed = match upper_cmd.as_bytes() {
                b"AUTHENTICATE" => ParsedCommand::Auth,
                b"LOGIN" => ParsedCommand::Login, // TODO parse username
                b"Enable" => ParsedCommand::Enable,
                b"SELECT" => ParsedCommand::Select,
                b"EXAMINE" => ParsedCommand::Examine,
                b"CREATE" => ParsedCommand::Create,
                b"DELETE" => ParsedCommand::Delete,
                b"RENAME" => ParsedCommand::Rename,
                b"SUBSCRIBE" => ParsedCommand::Subscribe,
                b"UBSUBSCRIBE" => ParsedCommand::Unsubscribe,
                b"LIST" => ParsedCommand::List,
                b"LSUB" => ParsedCommand::Lsub,
                b"STATUS" => ParsedCommand::Status,
                b"APPEND" => ParsedCommand::Append,
                b"SEARCH" => ParsedCommand::Search,
                b"FETCH" => ParsedCommand::Fetch,
                b"STORE" => ParsedCommand::Store,
                b"COPY" => ParsedCommand::Copy,
                b"MOVE" => ParsedCommand::Move,
                b"UID" => ParsedCommand::Uid,
                b"ID" => ParsedCommand::Id,
                _ => {
                    trace!("unknown IMAP command: {tag} {upper_cmd} ...");
                    ParsedCommand::Unknown
                }
            };

            Ok(Command {
                tag: SmolStr::from(tag),
                parsed,
                literal_arg,
            })
        } else {
            // commands without params
            let cmd = str::from_utf8(left).map_err(CommandLineError::InvalidUtf8Command)?;
            let upper_cmd = cmd.to_uppercase();

            let data = match upper_cmd.as_bytes() {
                b"CAPABILITY" => ParsedCommand::Capability,
                b"NOOP" => ParsedCommand::NoOperation,
                b"LOGOUT" => ParsedCommand::Logout,
                b"STARTTLS" => ParsedCommand::StartTls,
                b"NAMESPACE" => ParsedCommand::Namespace,
                b"IDLE" => ParsedCommand::Idle,
                b"CLOSE" => ParsedCommand::Close,
                b"UNSELECT" => ParsedCommand::Unselect,
                b"EXPUNGE" => ParsedCommand::Expunge,
                _ => {
                    trace!("unknown IMAP command: {tag} {upper_cmd}");
                    ParsedCommand::Unknown
                }
            };

            Ok(Command {
                tag: SmolStr::from(tag),
                parsed: data,
                literal_arg: None,
            })
        }
    }

    pub fn parse_continue_line(&mut self, line: &[u8]) -> Result<(), CommandLineError> {
        let left = line
            .strip_suffix(b"\r\n")
            .ok_or(CommandLineError::NoTrailingSequence)?;

        #[cfg(debug_assertions)]
        if let Ok(s) = str::from_utf8(left) {
            trace!("[IMAP] +-> {s}");
        }

        if left.is_empty() {
            self.literal_arg = None;
        } else {
            self.literal_arg = LiteralArgument::check(left)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn capability() {
        let cmd = Command::parse_line(b"a441 CAPABILITY\r\n").unwrap();
        assert_eq!(cmd.tag.as_str(), "a441");
        assert_eq!(cmd.parsed, ParsedCommand::Capability);
        assert!(cmd.literal_arg.is_none());
    }

    #[test]
    fn append() {
        let cmd = Command::parse_line(b"A003 APPEND saved-messages (\\Seen) {326}\r\n").unwrap();
        assert_eq!(cmd.tag.as_str(), "A003");
        assert_eq!(cmd.parsed, ParsedCommand::Append);
        let literal = cmd.literal_arg.unwrap();
        assert!(literal.wait_continuation);
        assert_eq!(literal.size, 326);

        let cmd = Command::parse_line(b"A003 APPEND saved-messages (\\Seen) {297+}\r\n").unwrap();
        assert_eq!(cmd.tag.as_str(), "A003");
        assert_eq!(cmd.parsed, ParsedCommand::Append);
        let literal = cmd.literal_arg.unwrap();
        assert!(!literal.wait_continuation);
        assert_eq!(literal.size, 297);
    }
}
