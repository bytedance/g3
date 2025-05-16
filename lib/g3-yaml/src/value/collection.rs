/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::anyhow;
use yaml_rust::Yaml;

use g3_types::collection::SelectivePickPolicy;

pub fn as_selective_pick_policy(value: &Yaml) -> anyhow::Result<SelectivePickPolicy> {
    if let Yaml::String(s) = value {
        let pick_policy =
            SelectivePickPolicy::from_str(s).map_err(|_| anyhow!("invalid pick policy"))?;
        Ok(pick_policy)
    } else {
        Err(anyhow!(
            "yaml value type for 'selective pick policy' should be 'string'"
        ))
    }
}
