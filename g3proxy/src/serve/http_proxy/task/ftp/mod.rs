/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use super::{CommonTaskContext, HttpProxyServerStats, protocol};

mod task;
pub(super) use task::FtpOverHttpTask;

mod stats;
use stats::{FtpOverHttpTaskCltWrapperStats, FtpOverHttpTaskStats};

mod connection;
use connection::HttpProxyFtpConnectionProvider;

mod list;
use list::{ChunkedListWriter, EndingListWriter, ListWriter};
