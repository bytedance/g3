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

use g3_dpi::Protocol;

use crate::serve::ServerTaskError;

#[derive(Debug, Error)]
pub(crate) enum InterceptionError {
    #[error("tls: {0}")]
    Tls(super::tls::TlsInterceptionError),
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
