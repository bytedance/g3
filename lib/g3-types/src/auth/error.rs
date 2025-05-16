/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;
use std::time::Duration;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum UserAuthError {
    #[error("no user is supplied")]
    NoUserSupplied,
    #[error("no such user found")]
    NoSuchUser,
    #[error("token not match")]
    TokenNotMatch,
    #[error("user has been expired")]
    ExpiredUser,
    #[error("user has been blocked")]
    BlockedUser(Duration),
    #[error("src addr {0} is blocked")]
    BlockedSrcIp(SocketAddr),
}

impl UserAuthError {
    pub fn blocked_delay(&self) -> Option<Duration> {
        if let UserAuthError::BlockedUser(duration) = self {
            if duration.is_zero() {
                None
            } else {
                Some(*duration)
            }
        } else {
            None
        }
    }
}

#[derive(Debug, Error)]
pub enum AuthParseError {
    #[error("unsupported auth type")]
    UnsupportedAuthType,
    #[error("invalid base64 encoding")]
    InvalidBase64Encoding,
    #[error("invalid utf-8 encoding")]
    InvalidUtf8Encoding,
    #[error("invalid username")]
    InvalidUsername,
    #[error("invalid password")]
    InvalidPassword,
    #[error("no delimiter found")]
    NoDelimiterFound,
}
