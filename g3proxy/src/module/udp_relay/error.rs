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
pub(crate) enum UdpRelaySetupError {
    #[error("method is not available")]
    MethodUnavailable,
    #[error("resolve failed: {0}")]
    ResolveFailed(#[from] ResolveError),
    #[error("setup socket failed: {0:?}")]
    SetupSocketFailed(io::Error),
}

impl From<UdpRelaySetupError> for ServerTaskError {
    fn from(e: UdpRelaySetupError) -> Self {
        match e {
            UdpRelaySetupError::MethodUnavailable => {
                ServerTaskError::ForbiddenByRule(ServerTaskForbiddenError::MethodUnavailable)
            }
            UdpRelaySetupError::ResolveFailed(e) => ServerTaskError::from(e),
            UdpRelaySetupError::SetupSocketFailed(_) => {
                ServerTaskError::InternalServerError("setup local udp socket failed")
            }
        }
    }
}
