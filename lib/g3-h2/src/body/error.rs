/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use thiserror::Error;

#[derive(Debug, Error)]
pub enum H2StreamBodyTransferError {
    #[error("recv data: {0}")]
    RecvData(h2::Error),
    #[error("wait send capacity: {0}")]
    WaitSendCapacity(h2::Error),
    #[error("sender not in send state")]
    SenderNotInSendState,
    #[error("send data: {0}")]
    SendData(h2::Error),
    #[error("release recv capacity: {0}")]
    ReleaseRecvCapacity(h2::Error),
    #[error("recv trailers: {0}")]
    RecvTrailers(h2::Error),
    #[error("send trailers: {0}")]
    SendTrailers(h2::Error),
    #[error("send end of stream: {0}")]
    SendEndOfStream(h2::Error),
}

impl H2StreamBodyTransferError {
    pub fn is_recv_error(&self) -> bool {
        matches!(
            self,
            H2StreamBodyTransferError::RecvData(_)
                | H2StreamBodyTransferError::ReleaseRecvCapacity(_)
                | H2StreamBodyTransferError::RecvTrailers(_)
        )
    }
}
