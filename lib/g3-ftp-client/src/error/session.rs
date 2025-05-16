/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use thiserror::Error;

use super::command::FtpCommandError;

pub(crate) enum FtpAuthStatus {
    NotLoggedIn,
    LoggedIn,
    NeedPassword,
    NeedAccount,
}

#[derive(Debug, Error)]
pub enum FtpSessionOpenError {
    #[error("raw command error: {0}")]
    RawCommandError(FtpCommandError),
    #[error("service not available")]
    ServiceNotAvailable,
    #[error("not logged in")]
    NotLoggedIn,
    #[error("extra account is needed")]
    AccountIsNeeded,
}

impl From<FtpCommandError> for FtpSessionOpenError {
    fn from(e: FtpCommandError) -> Self {
        match e {
            FtpCommandError::ServiceNotAvailable => FtpSessionOpenError::ServiceNotAvailable,
            _ => FtpSessionOpenError::RawCommandError(e),
        }
    }
}
