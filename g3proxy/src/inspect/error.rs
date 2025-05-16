/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use thiserror::Error;

use g3_dpi::Protocol;

use crate::serve::ServerTaskError;

#[derive(Debug, Error)]
pub(crate) enum InterceptionError {
    #[error("tls: {0}")]
    Tls(super::tls::TlsInterceptionError),
    #[error("start tls: {0}")]
    StartTls(super::tls::TlsInterceptionError),
    #[error("http1: {0}")]
    H1(super::http::H1InterceptionError),
    #[error("http2: {0}")]
    H2(super::http::H2InterceptionError),
}

impl InterceptionError {
    pub(super) fn into_server_task_error(self, protocol: Protocol) -> ServerTaskError {
        ServerTaskError::InterceptionError(protocol, self)
    }
}
