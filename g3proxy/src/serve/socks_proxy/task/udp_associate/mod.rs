/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use super::{CommonTaskContext, SocksProxyServerStats};

mod task;
pub(super) use task::SocksProxyUdpAssociateTask;

mod recv;
mod send;
mod stats;

use recv::Socks5UdpAssociateClientRecv;
use send::Socks5UdpAssociateClientSend;
use stats::{UdpAssociateTaskCltWrapperStats, UdpAssociateTaskStats};
