/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

mod stats;
use stats::HttpRProxyServerStats;

mod task;

mod server;
pub(super) use server::HttpRProxyServer;

mod host;
use host::HttpHost;
