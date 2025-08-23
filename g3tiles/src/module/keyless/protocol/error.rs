/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::io;

use thiserror::Error;

#[derive(Debug, Error)]
pub(crate) enum KeylessRecvMessageError {
    #[error("io failed: {0:?}")]
    IoFailed(#[from] io::Error),
    #[error("io closed")]
    IoClosed,
    #[error("invalid header length {0}")]
    InvalidHeaderLength(usize),
    #[error("invalid payload length, only received {0} of {1}")]
    InvalidPayloadLength(usize, usize),
}
