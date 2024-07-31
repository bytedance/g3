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

use std::str::{self, Utf8Error};

use atoi::FromRadix10Checked;
use smol_str::SmolStr;
use thiserror::Error;

mod bye;
pub use bye::ByeResponse;

#[derive(Debug, Error)]
pub enum ResponseLineError {
    #[error("no trailing sequence")]
    NoTrailingSequence,
    #[error("no tag found as a prefix")]
    NotTagPrefixed,
    #[error("invalid utf-8 response: {0}")]
    InvalidUtf8Response(Utf8Error),
    #[error("no result field found")]
    NoResultField,
    #[error("invalid tagged result")]
    InvalidTaggedResult,
    #[error("unknown untagged result")]
    UnknownUntaggedResult,
    #[error("invalid literal size")]
    InvalidLiteralSize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandResult {
    Success,
    Fail,
    ProtocolError,
}

pub struct TaggedResponse {
    pub tag: SmolStr,
    pub result: CommandResult,
}

impl TaggedResponse {
    fn parse(tag: &[u8], left: &[u8]) -> Result<Self, ResponseLineError> {
        let tag = str::from_utf8(tag).map_err(ResponseLineError::InvalidUtf8Response)?;
        let tag = SmolStr::from(tag);

        let Some(d) = memchr::memchr(b' ', left) else {
            return Err(ResponseLineError::NoResultField);
        };
        let result = str::from_utf8(&left[..d]).map_err(ResponseLineError::InvalidUtf8Response)?;
        let result = match result.to_uppercase().as_str() {
            "OK" => CommandResult::Success,
            "NO" => CommandResult::Fail,
            "BAD" => CommandResult::ProtocolError,
            _ => return Err(ResponseLineError::InvalidTaggedResult),
        };
        Ok(TaggedResponse { tag, result })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServerStatus {
    Information,
    Warning,
    Error,
    Authenticated,
    Close,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandData {
    Enabled,
    Capability,
    Fetch,
    Other,
}

pub struct UntaggedResponse {
    pub command_data: CommandData,
    pub literal_data: Option<usize>,
}

pub enum Response {
    CommandResult(TaggedResponse),
    ServerStatus(ServerStatus),
    CommandData(UntaggedResponse),
    ContinuationRequest,
}

impl Response {
    pub fn parse_line(line: &[u8]) -> Result<Self, ResponseLineError> {
        let left = line
            .strip_suffix(b"\r\n")
            .ok_or(ResponseLineError::NoTrailingSequence)?;

        let Some(d) = memchr::memchr(b' ', left) else {
            return Err(ResponseLineError::NotTagPrefixed);
        };

        match left[0] {
            b' ' => Err(ResponseLineError::NotTagPrefixed),
            b'*' => Self::parse_untagged(&left[d + 1..]),
            b'+' => Ok(Response::ContinuationRequest),
            _ => TaggedResponse::parse(&left[..d], &left[d + 1..]).map(Response::CommandResult),
        }
    }

    fn parse_untagged(left: &[u8]) -> Result<Self, ResponseLineError> {
        let Some(d) = memchr::memchr(b' ', left) else {
            return Err(ResponseLineError::NoResultField);
        };
        let result = str::from_utf8(&left[..d]).map_err(ResponseLineError::InvalidUtf8Response)?;
        match result.to_uppercase().as_str() {
            "OK" => Ok(Response::ServerStatus(ServerStatus::Information)),
            "NO" => Ok(Response::ServerStatus(ServerStatus::Warning)),
            "BAD" => Ok(Response::ServerStatus(ServerStatus::Error)),
            "PREAUTH" => Ok(Response::ServerStatus(ServerStatus::Authenticated)),
            "BYE" => Ok(Response::ServerStatus(ServerStatus::Close)),
            "ENABLED" => Ok(Response::CommandData(UntaggedResponse {
                command_data: CommandData::Enabled,
                literal_data: None,
            })),
            "CAPABILITY" => Ok(Response::CommandData(UntaggedResponse {
                command_data: CommandData::Capability,
                literal_data: None,
            })),
            "LIST" | "NAMESPACE" | "STATUS" | "SEARCH" | "ESEARCH" | "FLAGS" => {
                Ok(Response::CommandData(UntaggedResponse {
                    command_data: CommandData::Other,
                    literal_data: None,
                }))
            }
            _ => {
                let left = &left[d + 1..];
                match memchr::memchr(b' ', left) {
                    Some(d) => {
                        let result = str::from_utf8(&left[..d])
                            .map_err(ResponseLineError::InvalidUtf8Response)?;
                        match result.to_uppercase().as_str() {
                            "FETCH" => {
                                let literal_data = check_literal_size(left)?;
                                Ok(Response::CommandData(UntaggedResponse {
                                    command_data: CommandData::Fetch,
                                    literal_data,
                                }))
                            }
                            _ => Err(ResponseLineError::UnknownUntaggedResult),
                        }
                    }
                    None => {
                        let result =
                            str::from_utf8(left).map_err(ResponseLineError::InvalidUtf8Response)?;
                        match result.to_uppercase().as_str() {
                            "EXISTS" | "EXPUNGE" => Ok(Response::CommandData(UntaggedResponse {
                                command_data: CommandData::Other,
                                literal_data: None,
                            })),
                            _ => Err(ResponseLineError::UnknownUntaggedResult),
                        }
                    }
                }
            }
        }
    }
}

fn check_literal_size(left: &[u8]) -> Result<Option<usize>, ResponseLineError> {
    if left.ends_with(b"}") {
        if let Some(p) = memchr::memrchr(b'{', left) {
            let size_s = &left[p + 1..left.len() - 1];
            let (size, offset) = usize::from_radix_10_checked(size_s);
            if offset != size_s.len() {
                return Err(ResponseLineError::InvalidLiteralSize);
            }
            return match size {
                Some(size) => Ok(Some(size)),
                None => Err(ResponseLineError::InvalidLiteralSize),
            };
        }
    }
    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bye() {
        let rsp = Response::parse_line(b"* BYE Autologout; idle for too long\r\n").unwrap();
        let Response::ServerStatus(status) = rsp else {
            panic!("parse failed")
        };
        assert_eq!(status, ServerStatus::Close);
    }

    #[test]
    fn capability() {
        let rsp = Response::parse_line(
            b"* CAPABILITY STARTTLS AUTH=GSSAPI IMAP4rev2 LOGINDISABLED XPIG-LATIN\r\n",
        )
        .unwrap();
        let Response::CommandData(r) = rsp else {
            panic!("parse failed")
        };
        assert_eq!(r.command_data, CommandData::Capability);
        assert!(r.literal_data.is_none());
    }

    #[test]
    fn exists() {
        let rsp = Response::parse_line(b"* 23 EXISTS\r\n").unwrap();
        let Response::CommandData(r) = rsp else {
            panic!("parse failed")
        };
        assert_eq!(r.command_data, CommandData::Other);
        assert!(r.literal_data.is_none());
    }

    #[test]
    fn fetch() {
        let rsp = Response::parse_line(b"* 12 FETCH (BODY[HEADER] {342}\r\n").unwrap();
        let Response::CommandData(r) = rsp else {
            panic!("parse failed")
        };
        assert_eq!(r.command_data, CommandData::Fetch);
        assert_eq!(r.literal_data, Some(342));
    }
}
