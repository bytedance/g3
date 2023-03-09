/*
 * Copyright 2023 ByteDance and/or its affiliates.
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
