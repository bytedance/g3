/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use yaml_rust::Yaml;

use crate::YamlDocPosition;

pub trait YamlMapCallback {
    fn type_name(&self) -> &'static str;
    fn parse_kv(
        &mut self,
        key: &str,
        value: &Yaml,
        doc: Option<&YamlDocPosition>,
    ) -> anyhow::Result<()>;

    fn check(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
}
