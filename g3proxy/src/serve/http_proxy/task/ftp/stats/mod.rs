/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use super::HttpProxyServerStats;

mod task;
pub(super) use task::FtpOverHttpTaskStats;

mod wrapper;
pub(super) use wrapper::FtpOverHttpTaskCltWrapperStats;
