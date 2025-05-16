/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use slog::Logger;

use g3_io_ext::{UdpRelayRemoteRecv, UdpRelayRemoteSend};

mod error;
mod stats;
mod task;

pub(crate) use error::UdpRelaySetupError;
pub(crate) use stats::{
    ArcUdpRelayTaskRemoteStats, UdpRelayRemoteWrapperStats, UdpRelayTaskRemoteStats,
};
pub(crate) use task::{UdpRelayTaskConf, UdpRelayTaskNotes};

pub(crate) type UdpRelaySetupResult = Result<
    (
        Box<dyn UdpRelayRemoteRecv + Unpin + Send + Sync>,
        Box<dyn UdpRelayRemoteSend + Unpin + Send + Sync>,
        Option<Logger>,
    ),
    UdpRelaySetupError,
>;
