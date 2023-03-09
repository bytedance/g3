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

use thiserror::Error;

use super::command::FtpCommandError;
use super::transfer::{FtpLineDataReadError, FtpTransferServerError, FtpTransferSetupError};
use crate::error::FtpRawResponseError;

#[derive(Debug)]
pub(crate) enum FtpFilePreTransferStatus {
    Proceed,
    Invalid,
}

#[derive(Debug, Error)]
pub enum FtpFileFactsParseError {
    #[error("no space delimiter")]
    NoSpaceDelimiter,
    #[error("no delimiter in fact ({0})")]
    NoDelimiterInFact(String),
    #[error("invalid modify time: {0}")]
    InvalidModifyTime(chrono::ParseError),
    #[error("invalid create time: {0}")]
    InvalidCreateTime(chrono::ParseError),
    #[error("invalid size")]
    InvalidSize,
}

#[derive(Debug, Error)]
pub enum FtpFileStatError {
    #[error("raw command error: {0}")]
    RawCommandError(FtpCommandError),
    #[error("service not available")]
    ServiceNotAvailable,
    #[error("feature unavailable")]
    FeatUnavailable,
    #[error("file unavailable")]
    FileUnavailable,
}

impl From<FtpCommandError> for FtpFileStatError {
    fn from(e: FtpCommandError) -> Self {
        match e {
            FtpCommandError::ServiceNotAvailable => FtpFileStatError::ServiceNotAvailable,
            _ => FtpFileStatError::RawCommandError(e),
        }
    }
}

#[derive(Debug, Error)]
pub enum FtpFileRetrieveStartError {
    #[error("data transfer setup error: {0}")]
    TransferSetupFailed(FtpTransferSetupError),
    #[error("command error: {0}")]
    CommandError(FtpCommandError),
    #[error("service not available")]
    ServiceNotAvailable,
    #[error("file unavailable")]
    FileUnavailable,
}

impl From<FtpCommandError> for FtpFileRetrieveStartError {
    fn from(e: FtpCommandError) -> Self {
        match e {
            FtpCommandError::ServiceNotAvailable => FtpFileRetrieveStartError::ServiceNotAvailable,
            _ => FtpFileRetrieveStartError::CommandError(e),
        }
    }
}

impl From<FtpTransferSetupError> for FtpFileRetrieveStartError {
    fn from(e: FtpTransferSetupError) -> Self {
        match e {
            FtpTransferSetupError::ServiceNotAvailable => {
                FtpFileRetrieveStartError::ServiceNotAvailable
            }
            _ => FtpFileRetrieveStartError::TransferSetupFailed(e),
        }
    }
}

#[derive(Debug, Error)]
pub enum FtpFileStoreStartError {
    #[error("data transfer setup error: {0}")]
    TransferSetupFailed(FtpTransferSetupError),
    #[error("command error: {0}")]
    CommandError(FtpCommandError),
    #[error("service not available")]
    ServiceNotAvailable,
    #[error("file unavailable")]
    FileUnavailable,
    #[error("need account for storing")]
    NeedAccountForStoring,
    #[error("filename not allowed")]
    FileNameNotAllowed,
    #[error("insufficient storage space")]
    InsufficientStorageSpace,
}

impl From<FtpCommandError> for FtpFileStoreStartError {
    fn from(e: FtpCommandError) -> Self {
        match e {
            FtpCommandError::ServiceNotAvailable => FtpFileStoreStartError::ServiceNotAvailable,
            _ => FtpFileStoreStartError::CommandError(e),
        }
    }
}

impl From<FtpTransferSetupError> for FtpFileStoreStartError {
    fn from(e: FtpTransferSetupError) -> Self {
        match e {
            FtpTransferSetupError::ServiceNotAvailable => {
                FtpFileStoreStartError::ServiceNotAvailable
            }
            _ => FtpFileStoreStartError::TransferSetupFailed(e),
        }
    }
}

#[derive(Debug, Error)]
pub enum FtpFileListError {
    #[error("server reported error: {0}")]
    ServerReportedError(#[from] FtpTransferServerError),
    #[error("timeout to wait end reply")]
    TimeoutToWaitEndReply,
    #[error("timeout to wait data eof")]
    TimeoutToWaitDataEof,
    #[error("timeout to wait all data")]
    TimeoutToWaitAllData,
    #[error("data read failed: {0}")]
    DataReadFailed(FtpLineDataReadError),
    #[error("local io callback failed")]
    LocalIoCallbackFailed,
}

impl From<FtpLineDataReadError> for FtpFileListError {
    fn from(e: FtpLineDataReadError) -> Self {
        if matches!(e, FtpLineDataReadError::AbortedByCallback) {
            FtpFileListError::DataReadFailed(e)
        } else {
            FtpFileListError::LocalIoCallbackFailed
        }
    }
}

#[derive(Debug, Error)]
pub enum FtpFileRetrieveError {
    #[error("server reported error: {0}")]
    ServerReportedError(FtpTransferServerError),
    #[error("timeout to wait end reply")]
    TimeoutToWaitEndReply,
    #[error("control read error: {0}")]
    ControlReadError(#[from] FtpRawResponseError),
}

impl From<FtpTransferServerError> for FtpFileRetrieveError {
    fn from(e: FtpTransferServerError) -> Self {
        if let FtpTransferServerError::RecvFailed(e) = e {
            FtpFileRetrieveError::ControlReadError(e)
        } else {
            FtpFileRetrieveError::ServerReportedError(e)
        }
    }
}

#[derive(Debug, Error)]
pub enum FtpFileStoreError {
    #[error("server reported error: {0}")]
    ServerReportedError(FtpTransferServerError),
    #[error("timeout to wait end reply")]
    TimeoutToWaitEndReply,
    #[error("control read error: {0}")]
    ControlReadError(#[from] FtpRawResponseError),
}

impl From<FtpTransferServerError> for FtpFileStoreError {
    fn from(e: FtpTransferServerError) -> Self {
        if let FtpTransferServerError::RecvFailed(e) = e {
            FtpFileStoreError::ControlReadError(e)
        } else {
            FtpFileStoreError::ServerReportedError(e)
        }
    }
}
