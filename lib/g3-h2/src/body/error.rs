/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use thiserror::Error;

#[derive(Debug, Error)]
pub enum H2StreamBodyTransferError {
    #[error("recv data failed: {0}")]
    RecvDataFailed(h2::Error),
    #[error("error while wait send capacity: {0}")]
    WaitSendCapacityFailed(h2::Error),
    #[error("sender not in send state")]
    SenderNotInSendState,
    #[error("send data failed: {0}")]
    SendDataFailed(h2::Error),
    #[error("error while release recv capacity: {0}")]
    ReleaseRecvCapacityFailed(h2::Error),
    #[error("recv trailers failed: {0}")]
    RecvTrailersFailed(h2::Error),
    #[error("send trailers failed: {0}")]
    SendTrailersFailed(h2::Error),
    #[error("error while set graceful end of stream: {0}")]
    GracefulCloseError(h2::Error),
}
