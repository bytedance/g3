/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;

use thiserror::Error;

use g3_resolver::ResolveError;

use crate::serve::{ServerTaskError, ServerTaskForbiddenError};

#[derive(Error, Debug)]
pub(crate) enum UdpRelaySetupError {
    #[error("method is not available")]
    MethodUnavailable,
    #[error("escaper is not usable: {0:?}")]
    EscaperNotUsable(anyhow::Error),
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
            UdpRelaySetupError::EscaperNotUsable(e) => ServerTaskError::EscaperNotUsable(e),
            UdpRelaySetupError::ResolveFailed(e) => ServerTaskError::from(e),
            UdpRelaySetupError::SetupSocketFailed(_) => {
                ServerTaskError::InternalServerError("setup local udp socket failed")
            }
        }
    }
}
