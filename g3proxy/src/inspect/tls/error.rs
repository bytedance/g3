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
pub(crate) enum TlsInterceptionError {
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
