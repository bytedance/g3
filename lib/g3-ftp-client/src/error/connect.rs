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
