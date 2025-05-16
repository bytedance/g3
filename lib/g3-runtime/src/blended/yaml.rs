/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use super::BlendedRuntimeConfig;

impl BlendedRuntimeConfig {
    pub fn parse_by_yaml_kv(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            "thread_number" => {
                let value = g3_yaml::value::as_usize(v)?;
                self.set_thread_number(value);
                Ok(())
            }
            "thread_name" => {
                let name = g3_yaml::value::as_ascii(v)
                    .context(format!("invalid ascii string value for key {k}"))?;
                self.set_thread_name(name.as_str());
                Ok(())
            }
            "thread_stack_size" => {
                let value = g3_yaml::humanize::as_usize(v)
                    .context(format!("invalid humanize usize value for key {k}"))?;
                self.set_thread_stack_size(value);
                Ok(())
            }
            "max_io_events_per_tick" => {
                let capacity = g3_yaml::value::as_usize(v)?;
                self.set_max_io_events_per_tick(capacity);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }
}
