/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use super::{CommonTaskContext, SocksProxyServerStats, tcp_connect, udp_associate, udp_connect};

mod task;
pub(crate) use task::SocksProxyNegotiationTask;

mod stats;
use stats::SocksProxyCltWrapperStats;
