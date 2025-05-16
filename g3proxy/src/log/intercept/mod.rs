/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use slog::Logger;

use g3_types::metrics::NodeName;

pub(crate) fn get_logger(auditor_name: &NodeName) -> Option<Logger> {
    super::audit::get_logger(super::LOG_TYPE_INTERCEPT, auditor_name)
}
