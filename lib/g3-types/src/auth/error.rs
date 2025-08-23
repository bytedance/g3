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

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::time::Duration;

    #[test]
    fn user_auth_error_variants() {
        let err = UserAuthError::NoUserSupplied;
        assert_eq!(err.to_string(), "no user is supplied");

        let err = UserAuthError::NoSuchUser;
        assert_eq!(err.to_string(), "no such user found");

        let err = UserAuthError::TokenNotMatch;
        assert_eq!(err.to_string(), "token not match");

        let err = UserAuthError::ExpiredUser;
        assert_eq!(err.to_string(), "user has been expired");

        let err = UserAuthError::BlockedUser(Duration::from_secs(10));
        assert_eq!(err.to_string(), "user has been blocked");

        let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();
        let err = UserAuthError::BlockedSrcIp(addr);
        assert_eq!(err.to_string(), "src addr 127.0.0.1:8080 is blocked");
    }

    #[test]
    fn user_auth_error_blocked_delay() {
        assert_eq!(UserAuthError::NoUserSupplied.blocked_delay(), None);
        assert_eq!(UserAuthError::NoSuchUser.blocked_delay(), None);
        assert_eq!(UserAuthError::TokenNotMatch.blocked_delay(), None);
        assert_eq!(UserAuthError::ExpiredUser.blocked_delay(), None);
        assert_eq!(
            UserAuthError::BlockedSrcIp("127.0.0.1:8080".parse().unwrap()).blocked_delay(),
            None
        );
        assert_eq!(
            UserAuthError::BlockedUser(Duration::ZERO).blocked_delay(),
            None
        );
        assert_eq!(
            UserAuthError::BlockedUser(Duration::from_secs(10)).blocked_delay(),
            Some(Duration::from_secs(10))
        );
    }

    #[test]
    fn auth_parse_error_variants() {
        let err = AuthParseError::UnsupportedAuthType;
        assert_eq!(err.to_string(), "unsupported auth type");

        let err = AuthParseError::InvalidBase64Encoding;
        assert_eq!(err.to_string(), "invalid base64 encoding");

        let err = AuthParseError::InvalidUtf8Encoding;
        assert_eq!(err.to_string(), "invalid utf-8 encoding");

        let err = AuthParseError::InvalidUsername;
        assert_eq!(err.to_string(), "invalid username");

        let err = AuthParseError::InvalidPassword;
        assert_eq!(err.to_string(), "invalid password");

        let err = AuthParseError::NoDelimiterFound;
        assert_eq!(err.to_string(), "no delimiter found");
    }
}
