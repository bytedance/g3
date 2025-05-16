/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use super::{CommonTaskContext, HttpRProxyServerStats, protocol};

mod task;
pub(super) use task::HttpRProxyUntrustedTask;

mod stats;
use stats::UntrustedCltReadWrapperStats;
