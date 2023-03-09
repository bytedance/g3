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

use tokio::io::{AsyncRead, AsyncWrite};

mod error;
mod task;

pub(crate) use error::TcpConnectError;
pub(crate) use task::TcpConnectTaskNotes;

pub(crate) type TcpConnection = (
    Box<dyn AsyncRead + Unpin + Send + Sync>,
    Box<dyn AsyncWrite + Unpin + Send + Sync>,
);
pub(crate) type TcpConnectResult = Result<TcpConnection, TcpConnectError>;
