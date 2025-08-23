/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod command;
mod connect;
mod file;
mod response;
mod session;
mod transfer;

pub(crate) use file::FtpFilePreTransferStatus;
pub(crate) use session::FtpAuthStatus;

pub use command::FtpCommandError;
pub use connect::FtpConnectError;
pub use file::{
    FtpFileFactsParseError, FtpFileListError, FtpFileRetrieveError, FtpFileRetrieveStartError,
    FtpFileStatError, FtpFileStoreError, FtpFileStoreStartError,
};
pub use response::FtpRawResponseError;
pub use session::FtpSessionOpenError;
pub use transfer::{FtpLineDataReadError, FtpTransferServerError, FtpTransferSetupError};
