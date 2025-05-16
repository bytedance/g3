/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use thiserror::Error;

use crate::error::FtpCommandError;

#[derive(Debug, Error)]
pub enum FtpConnectError<E: std::error::Error> {
    #[error("connect failed: {0:?}")]
    ConnectIoError(E),
    #[error("timed out to connect")]
    ConnectTimedOut,
    #[error("timed out to receive greetings")]
    GreetingTimedOut,
    #[error("greeting failed: {0}")]
    GreetingFailed(FtpCommandError),
    #[error("negotiation failed: {0}")]
    NegotiationFailed(FtpCommandError),
    #[error("service not available")]
    ServiceNotAvailable,
    #[error("invalid reply code {0}")]
    InvalidReplyCode(u16),
}
