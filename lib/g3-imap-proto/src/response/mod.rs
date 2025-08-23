/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::str::{self, Utf8Error};

use atoi::FromRadix10Checked;
use log::trace;
use smol_str::SmolStr;
use thiserror::Error;

mod bad;
pub use bad::BadResponse;

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
    Id,
    Other,
}

pub struct UntaggedResponse {
    pub command_data: CommandData,
    pub literal_data: Option<u64>,
}

impl UntaggedResponse {
    pub fn parse_continue_line(&mut self, line: &[u8]) -> Result<(), ResponseLineError> {
        let left = line
            .strip_suffix(b"\r\n")
            .ok_or(ResponseLineError::NoTrailingSequence)?;

        #[cfg(debug_assertions)]
        if let Ok(s) = str::from_utf8(left) {
            trace!("[IMAP] +-< {s}");
        }

        if left.is_empty() {
            self.literal_data = None;
        } else {
            self.literal_data = check_literal_size(left)?;
        }

        Ok(())
    }
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

        #[cfg(debug_assertions)]
        if let Ok(s) = str::from_utf8(left) {
            trace!("[IMAP] --< {s}");
        }

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
        match memchr::memchr(b' ', left) {
            Some(d) => {
                let r1 =
                    str::from_utf8(&left[..d]).map_err(ResponseLineError::InvalidUtf8Response)?;
                match r1.to_uppercase().as_str() {
                    "OK" => Ok(Response::ServerStatus(ServerStatus::Information)),
                    "NO" => Ok(Response::ServerStatus(ServerStatus::Warning)),
                    "BAD" => Ok(Response::ServerStatus(ServerStatus::Error)),
                    "PREAUTH" => Ok(Response::ServerStatus(ServerStatus::Authenticated)),
                    "BYE" => Ok(Response::ServerStatus(ServerStatus::Close)),
                    "ENABLED" => {
                        // rfc5161, rev2
                        Ok(Response::CommandData(UntaggedResponse {
                            command_data: CommandData::Enabled,
                            literal_data: None,
                        }))
                    }
                    "CAPABILITY" => Ok(Response::CommandData(UntaggedResponse {
                        command_data: CommandData::Capability,
                        literal_data: None,
                    })),
                    "ID" => {
                        // rfc2971, rev2
                        Ok(Response::CommandData(UntaggedResponse {
                            command_data: CommandData::Id,
                            literal_data: None,
                        }))
                    },
                    "LIST"
                    | "LSUB" // rev1
                    | "NAMESPACE" // rfc2342, rev2
                    | "STATUS" | "SEARCH"
                    | "ESEARCH" // rfc4731, rev2
                    | "FLAGS" => {
                        Ok(Response::CommandData(UntaggedResponse {
                            command_data: CommandData::Other,
                            literal_data: None,
                        }))
                    }
                    "SORT" | "THREAD" => {
                        // rfc5256
                        Ok(Response::CommandData(UntaggedResponse {
                            command_data: CommandData::Other,
                            literal_data: None,
                        }))
                    },
                    "LANGUAGE" | "COMPARATOR" => {
                        // rfc5255
                        Ok(Response::CommandData(UntaggedResponse {
                            command_data: CommandData::Other,
                            literal_data: None,
                        }))
                    },
                    "VANISHED" => {
                        // rfc7162
                        Ok(Response::CommandData(UntaggedResponse {
                            command_data: CommandData::Other,
                            literal_data: None,
                        }))
                    }
                    "QUOTA" | "QUOTAROOT" => {
                        // rfc9208
                        Ok(Response::CommandData(UntaggedResponse {
                            command_data: CommandData::Other,
                            literal_data: None,
                        }))
                    },
                    "ACL" | "LISTRIGHTS" | "MYRIGHTS" => {
                        // rfc4314
                        Ok(Response::CommandData(UntaggedResponse {
                            command_data: CommandData::Other,
                            literal_data: None,
                        }))
                    }
                    "CONVERSION" | "CONVERTED" => {
                        // rfc5259
                        Ok(Response::CommandData(UntaggedResponse {
                            command_data: CommandData::Other,
                            literal_data: None,
                        }))
                    }
                    "METADATA" => {
                        // rfc5464
                        Ok(Response::CommandData(UntaggedResponse {
                            command_data: CommandData::Other,
                            literal_data: None,
                        }))
                    }
                    "GENURLAUTH" | "URLFETCH" => {
                        // rfc4467
                        Ok(Response::CommandData(UntaggedResponse {
                            command_data: CommandData::Other,
                            literal_data: None,
                        }))
                    }
                    _ => {
                        let left = &left[d + 1..];
                        match memchr::memchr(b' ', left) {
                            Some(d) => {
                                let r2 = str::from_utf8(&left[..d])
                                    .map_err(ResponseLineError::InvalidUtf8Response)?;
                                match r2.to_uppercase().as_str() {
                                    "FETCH" => {
                                        let literal_data = check_literal_size(left)?;
                                        Ok(Response::CommandData(UntaggedResponse {
                                            command_data: CommandData::Fetch,
                                            literal_data,
                                        }))
                                    }
                                    _ => {
                                        trace!("unknown IMAP response line: * {r1} {r2} ...");
                                        Err(ResponseLineError::UnknownUntaggedResult)
                                    }
                                }
                            }
                            None => {
                                let r2 = str::from_utf8(left)
                                    .map_err(ResponseLineError::InvalidUtf8Response)?;
                                match r2.to_uppercase().as_str() {
                                    "EXISTS" | "EXPUNGE" | "RECENT" => {
                                        Ok(Response::CommandData(UntaggedResponse {
                                            command_data: CommandData::Other,
                                            literal_data: None,
                                        }))
                                    }
                                    _ => {
                                        trace!("unknown IMAP response line: * {r1} {r2}");
                                        Err(ResponseLineError::UnknownUntaggedResult)
                                    }
                                }
                            }
                        }
                    }
                }
            }
            None => {
                let r1 = str::from_utf8(left).map_err(ResponseLineError::InvalidUtf8Response)?;
                match r1.to_uppercase().as_str() {
                    "SEARCH" => Ok(Response::CommandData(UntaggedResponse {
                        command_data: CommandData::Other,
                        literal_data: None,
                    })),
                    _ => {
                        trace!("unknown IMAP response line: * {r1}");
                        Err(ResponseLineError::UnknownUntaggedResult)
                    }
                }
            }
        }
    }
}

fn check_literal_size(left: &[u8]) -> Result<Option<u64>, ResponseLineError> {
    if left.ends_with(b"}")
        && let Some(p) = memchr::memrchr(b'{', left)
    {
        let size_s = &left[p + 1..left.len() - 1];
        let (size, offset) = u64::from_radix_10_checked(size_s);
        if offset != size_s.len() {
            return Err(ResponseLineError::InvalidLiteralSize);
        }
        return match size {
            Some(size) => Ok(Some(size)),
            None => Err(ResponseLineError::InvalidLiteralSize),
        };
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
