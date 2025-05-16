/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use slog::Logger;

use g3_io_ext::{UdpCopyRemoteRecv, UdpCopyRemoteSend};

mod error;
mod stats;
mod task;

pub(crate) use error::UdpConnectError;
pub(crate) use stats::{
    ArcUdpConnectTaskRemoteStats, UdpConnectRemoteWrapperStats, UdpConnectTaskRemoteStats,
};
pub(crate) use task::{UdpConnectTaskConf, UdpConnectTaskNotes};

pub(crate) type UdpConnectResult = Result<
    (
        Box<dyn UdpCopyRemoteRecv + Unpin + Send + Sync>,
        Box<dyn UdpCopyRemoteSend + Unpin + Send + Sync>,
        Option<Logger>,
    ),
    UdpConnectError,
>;
