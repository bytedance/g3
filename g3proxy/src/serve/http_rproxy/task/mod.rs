/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use super::HttpRProxyServerStats;
use crate::config::server::http_rproxy::HttpRProxyServerConfig;

mod common;
pub(super) use common::CommonTaskContext;

mod protocol;

mod forward;
mod pipeline;
mod untrusted;

use forward::HttpRProxyForwardTask;
pub(super) use pipeline::{
    HttpRProxyPipelineReaderTask, HttpRProxyPipelineStats, HttpRProxyPipelineWriterTask,
};
use untrusted::HttpRProxyUntrustedTask;
