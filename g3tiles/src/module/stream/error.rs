/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::io;

use thiserror::Error;

use g3_types::net::ConnectError;

#[derive(Debug, Error)]
pub(crate) enum StreamConnectError {
    #[error("upstream not resolved")]
    UpstreamNotResolved,
    #[error("setup socket failed: {0:?}")]
    SetupSocketFailed(io::Error),
    #[error("connect failed: {0}")]
    ConnectFailed(#[from] ConnectError),
}
