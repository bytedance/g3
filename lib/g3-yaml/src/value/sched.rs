/*
 * Copyright 2023 ByteDance and/or its affiliates.
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

use anyhow::{anyhow, Context};
use nix::sched::CpuSet;
use yaml_rust::Yaml;

pub fn as_cpu_set(v: &Yaml) -> anyhow::Result<CpuSet> {
    let mut set = CpuSet::new();

    if let Yaml::Array(seq) = v {
        for (i, v) in seq.iter().enumerate() {
            let id = crate::value::as_usize(v).context(format!("invalid cpu id value #{i}"))?;
            set.set(id)
                .map_err(|e| anyhow!("unable to add cpu {id} to this set: {}", e.desc()))?;
        }
    } else {
        let id = crate::value::as_usize(v).context("invalid cpu id value")?;
        set.set(id)
            .map_err(|e| anyhow!("unable to add cpu {id} to this set: {}", e.desc()))?;
    }

    Ok(set)
}
