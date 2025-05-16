/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use openssl::x509::X509VerifyResult;
use slog::{Record, Serializer, Value};

pub struct LtX509VerifyResult(pub X509VerifyResult);

impl Value for LtX509VerifyResult {
    fn serialize(
        &self,
        _record: &Record,
        key: slog::Key,
        serializer: &mut dyn Serializer,
    ) -> slog::Result {
        serializer.emit_str(key, self.0.error_string())
    }
}
