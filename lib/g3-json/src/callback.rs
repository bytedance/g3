/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use serde_json::Value;

pub trait JsonMapCallback {
    fn type_name(&self) -> &'static str;
    fn parse_kv(&mut self, key: &str, value: &Value) -> anyhow::Result<()>;

    fn check(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
}
