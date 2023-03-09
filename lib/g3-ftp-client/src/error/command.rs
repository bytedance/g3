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
