/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use chrono::{DateTime, SecondsFormat, Utc};
use slog::{Record, Serializer, Value};

pub struct LtDateTime<'a>(pub &'a DateTime<Utc>);

impl Value for LtDateTime<'_> {
    fn serialize(
        &self,
        _record: &Record,
        key: slog::Key,
        serializer: &mut dyn Serializer,
    ) -> slog::Result {
        let s = self.0.to_rfc3339_opts(SecondsFormat::Micros, true);
        serializer.emit_str(key, &s)
    }
}
