/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 G3-OSS developers.
 */

use arcstr::ArcStr;
use slog::{Record, Serializer, Value};

pub struct LtUserName<'a>(pub &'a ArcStr);

impl Value for LtUserName<'_> {
    fn serialize(
        &self,
        _record: &Record,
        key: slog::Key,
        serializer: &mut dyn Serializer,
    ) -> slog::Result {
        serializer.emit_str(key, self.0.as_str())
    }
}
