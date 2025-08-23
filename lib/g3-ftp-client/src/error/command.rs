/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;

use thiserror::Error;

use super::FtpRawResponseError;
use crate::control::FtpCommand;

#[derive(Debug, Error)]
pub enum FtpCommandError {
    #[error("not logged in")]
    NotLoggedIn,
    #[error("unable to send command: {0:?}")]
    SendFailed(io::Error),
    #[error("unable to recv reply: {0}")]
    RecvFailed(#[from] FtpRawResponseError),
    #[error("service not available")]
    ServiceNotAvailable,
    #[error("{0} syntax rejected by server")]
    RejectedCommandSyntax(FtpCommand),
    #[error("command {0} is not implemented by server")]
    CommandNotImplemented(FtpCommand),
    #[error("parameter is not implemented for command {0}")]
    ParameterNotImplemented(FtpCommand),
    #[error("unexpected reply code ({0} -> {1})")]
    UnexpectedReplyCode(FtpCommand, u16),
    #[error("invalid reply {1} syntax to command {0}")]
    InvalidReplySyntax(FtpCommand, u16),
    #[error("bad sequence of command {0}")]
    BadCommandSequence(FtpCommand),
    #[error("pre transfer failed for command {0} with reply code {1}")]
    PreTransferFailed(FtpCommand, u16),
}
