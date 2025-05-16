/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use super::SocksProxyServerStats;
use crate::config::server::socks_proxy::SocksProxyServerConfig;

mod common;
pub(super) use common::CommonTaskContext;

mod negotiation;
mod tcp_connect;
mod udp_associate;
mod udp_connect;

pub(super) use negotiation::SocksProxyNegotiationTask;
