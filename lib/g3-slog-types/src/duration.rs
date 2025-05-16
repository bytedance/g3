/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::time::Duration;

use slog::{Record, Serializer, Value};

pub struct LtDuration(pub Duration);

impl Value for LtDuration {
    fn serialize(
        &self,
        _record: &Record,
        key: slog::Key,
        serializer: &mut dyn Serializer,
    ) -> slog::Result {
        if self.0.is_zero() {
            serializer.emit_none(key)
        } else {
            serializer.emit_arguments(key, &format_args!("{:.3?}", self.0))
        }
    }
}
