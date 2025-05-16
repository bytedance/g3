/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_compat::CpuAffinity;

fn cpu_affinity_add_value(set: &mut CpuAffinity, v: &Yaml) -> anyhow::Result<()> {
    match v {
        Yaml::String(s) => set
            .parse_add(s)
            .map_err(|e| anyhow!("invalid CPU ID(s) string value {s}: {e}")),
        Yaml::Integer(i) => {
            let n = usize::try_from(*i)
                .map_err(|e| anyhow!("invalid CPU ID integer value {}: {e}", *i))?;
            set.add_id(n)
                .map_err(|e| anyhow!("invalid CPU ID {n}: {e}"))
        }
        _ => Err(anyhow!("invalid yaml value type for CPU ID list")),
    }
}

pub fn as_cpu_set(v: &Yaml) -> anyhow::Result<CpuAffinity> {
    let mut set = CpuAffinity::default();

    if let Yaml::Array(seq) = v {
        for (i, v) in seq.iter().enumerate() {
            cpu_affinity_add_value(&mut set, v)
                .context(format!("invalid CPU ID list value for #{i}"))?;
        }
    } else {
        cpu_affinity_add_value(&mut set, v)?;
    }

    Ok(set)
}
