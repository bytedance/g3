/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use slog::{Record, Serializer, Value};
use uuid::Uuid;

pub struct LtUuid<'a>(pub &'a Uuid);

impl Value for LtUuid<'_> {
    fn serialize(
        &self,
        _record: &Record,
        key: slog::Key,
        serializer: &mut dyn Serializer,
    ) -> slog::Result {
        serializer.emit_arguments(key, &format_args!("{}", self.0.simple()))
    }
}
