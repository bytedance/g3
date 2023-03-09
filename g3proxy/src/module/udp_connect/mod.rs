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

use slog::Logger;

use g3_io_ext::{UdpCopyRemoteRecv, UdpCopyRemoteSend};

mod error;
mod stats;
mod task;

pub(crate) use error::UdpConnectError;
pub(crate) use stats::{ArcUdpConnectTaskRemoteStats, UdpConnectTaskRemoteStats};
pub(crate) use task::UdpConnectTaskNotes;

pub(crate) type UdpConnectResult = Result<
    (
        Box<dyn UdpCopyRemoteRecv + Unpin + Send + Sync>,
        Box<dyn UdpCopyRemoteSend + Unpin + Send + Sync>,
        Logger,
    ),
    UdpConnectError,
>;
