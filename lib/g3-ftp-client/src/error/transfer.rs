/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;

use thiserror::Error;

use super::{FtpCommandError, FtpRawResponseError};
use crate::control::FtpCommand;

#[derive(Debug, Error)]
pub enum FtpTransferSetupError {
    #[error("raw command error: {0}")]
    RawCommandError(FtpCommandError),
    #[error("service not available")]
    ServiceNotAvailable,
    #[error("active data transfer is needed")]
    NeedActiveDataTransfer,
    #[error("data transfer not connected")]
    DataTransferNotConnected,
    #[error("data transfer connect timeout")]
    DataTransferConnectTimeout,
}

impl FtpTransferSetupError {
    pub(crate) fn skip_retry(&self) -> bool {
        matches!(self, FtpTransferSetupError::ServiceNotAvailable)
    }
}

impl From<FtpCommandError> for FtpTransferSetupError {
    fn from(e: FtpCommandError) -> Self {
        match e {
            FtpCommandError::ServiceNotAvailable => FtpTransferSetupError::ServiceNotAvailable,
            _ => FtpTransferSetupError::RawCommandError(e),
        }
    }
}

#[derive(Debug, Error)]
pub enum FtpTransferServerError {
    #[error("recv failed: {0}")]
    RecvFailed(#[from] FtpRawResponseError),
    #[error("data transfer not established")]
    DataTransferNotEstablished,
    #[error("data transfer lost")]
    DataTransferLost,
    #[error("server failed")]
    ServerFailed,
    #[error("restart needed")]
    RestartNeeded,
    #[error("page type unknown")]
    PageTypeUnknown,
    #[error("exceeded storage allocation")]
    ExceededStorageAllocation,
    #[error("unexpected end reply code ({0} -> {1})")]
    UnexpectedEndReplyCode(FtpCommand, u16),
}

#[derive(Debug, Error)]
pub enum FtpLineDataReadError {
    #[error("io failed: {0:?}")]
    IoFailed(#[from] io::Error),
    #[error("line {0} is too long")]
    LineTooLong(usize),
    #[error("unsupported encoding")]
    UnsupportedEncoding,
    #[error("too many lines")]
    TooManyLines,
    #[error("aborted by callback")]
    AbortedByCallback,
}
