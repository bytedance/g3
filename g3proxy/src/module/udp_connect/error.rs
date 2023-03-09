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

use g3_resolver::ResolveError;

use crate::serve::{ServerTaskError, ServerTaskForbiddenError};

#[derive(Error, Debug)]
pub(crate) enum UdpConnectError {
    #[error("method is not available")]
    MethodUnavailable,
    #[error("escaper is not usable")]
    EscaperNotUsable,
    #[error("no upstream addr supplied")]
    NoUpstreamSupplied,
    #[error("forbidden remote address")]
    ForbiddenRemoteAddress,
    #[error("resolve failed: {0}")]
    ResolveFailed(#[from] ResolveError),
    #[error("setup socket failed: {0:?}")]
    SetupSocketFailed(io::Error),
}

impl From<UdpConnectError> for ServerTaskError {
    fn from(e: UdpConnectError) -> Self {
        match e {
            UdpConnectError::MethodUnavailable => {
                ServerTaskError::ForbiddenByRule(ServerTaskForbiddenError::MethodUnavailable)
            }
            UdpConnectError::EscaperNotUsable => ServerTaskError::EscaperNotUsable,
            UdpConnectError::NoUpstreamSupplied => {
                ServerTaskError::InternalServerError("no upstream addr supplied")
            }
            UdpConnectError::ForbiddenRemoteAddress => {
                ServerTaskError::ForbiddenByRule(ServerTaskForbiddenError::IpBlocked)
            }
            UdpConnectError::ResolveFailed(e) => ServerTaskError::from(e),
            UdpConnectError::SetupSocketFailed(_) => {
                ServerTaskError::InternalServerError("setup local udp socket failed")
            }
        }
    }
}
