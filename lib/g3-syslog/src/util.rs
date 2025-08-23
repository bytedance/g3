/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use slog::Level;

use crate::types::{Facility, Priority, Severity};

pub(crate) fn level_to_severity(level: Level) -> Severity {
    match level {
        Level::Critical => Severity::Critical,
        Level::Error => Severity::Error,
        Level::Warning => Severity::Warning,
        Level::Info => Severity::Notice,
        Level::Debug => Severity::Info,
        Level::Trace => Severity::Debug,
    }
}

pub(crate) fn encode_priority(severity: Severity, facility: Facility) -> Priority {
    facility as u8 | severity as u8
}
