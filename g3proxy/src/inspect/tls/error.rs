/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum TlsInterceptionError {
    #[error("internal openssl server error: {0}")]
    InternalOpensslServerError(anyhow::Error),
    #[error("client handshake timeout")]
    ClientHandshakeTimeout,
    #[error("client handshake failed: {0:?}")]
    ClientHandshakeFailed(anyhow::Error),
    #[error("upstream prepare failed: {0:?}")]
    UpstreamPrepareFailed(anyhow::Error),
    #[error("upstream handshake timeout")]
    UpstreamHandshakeTimeout,
    #[error("upstream handshake failed: {0:?}")]
    UpstreamHandshakeFailed(anyhow::Error),
    #[error("no fake cert generated: {0:?}")]
    NoFakeCertGenerated(anyhow::Error),
}
