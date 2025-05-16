/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use super::HttpProxyServerStats;
use crate::config::server::http_proxy::HttpProxyServerConfig;

mod common;
pub(super) use common::CommonTaskContext;

mod protocol;

mod connect;
mod forward;
mod ftp;
mod pipeline;
mod untrusted;

use connect::HttpProxyConnectTask;
use forward::HttpProxyForwardTask;
use ftp::FtpOverHttpTask;
pub(super) use pipeline::{
    HttpProxyPipelineReaderTask, HttpProxyPipelineStats, HttpProxyPipelineWriterTask,
};
use untrusted::HttpProxyUntrustedTask;
