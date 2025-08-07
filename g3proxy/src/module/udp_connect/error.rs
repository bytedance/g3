/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::net::SocketAddr;

use thiserror::Error;

use g3_resolver::ResolveError;

use crate::serve::{ServerTaskError, ServerTaskForbiddenError};

#[derive(Error, Debug)]
pub(crate) enum UdpConnectError {
    #[error("method is not available")]
    MethodUnavailable,
    #[error("escaper is not usable: {0:?}")]
    EscaperNotUsable(anyhow::Error),
    #[error("forbidden remote address {0}")]
    ForbiddenRemoteAddress(SocketAddr),
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
            UdpConnectError::EscaperNotUsable(e) => ServerTaskError::EscaperNotUsable(e),
            UdpConnectError::ForbiddenRemoteAddress(_) => {
                ServerTaskError::ForbiddenByRule(ServerTaskForbiddenError::IpBlocked)
            }
            UdpConnectError::ResolveFailed(e) => ServerTaskError::from(e),
            UdpConnectError::SetupSocketFailed(_) => {
                ServerTaskError::InternalServerError("setup local udp socket failed")
            }
        }
    }
}
