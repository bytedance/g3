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

use thiserror::Error;

#[derive(Debug, Error)]
pub enum H2StreamBodyTransferError {
    #[error("recv data failed: {0}")]
    RecvDataFailed(h2::Error),
    #[error("error while wait send capacity: {0}")]
    WaitSendCapacityFailed(h2::Error),
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
