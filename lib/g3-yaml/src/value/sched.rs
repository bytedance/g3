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

use yaml_rust::Yaml;

use g3_compat::CpuAffinity;

#[cfg(not(target_os = "macos"))]
pub fn as_cpu_set(v: &Yaml) -> anyhow::Result<CpuAffinity> {
    use anyhow::{anyhow, Context};

    let mut set = CpuAffinity::default();

    if let Yaml::Array(seq) = v {
        for (i, v) in seq.iter().enumerate() {
            let id = crate::value::as_usize(v).context(format!("invalid cpu id value #{i}"))?;
            set.add_id(id)
                .map_err(|e| anyhow!("unable to add cpu {id} to this set: {e}"))?;
        }
    } else {
        let id = crate::value::as_usize(v).context("invalid cpu id value")?;
        set.add_id(id)
            .map_err(|e| anyhow!("unable to add cpu {id} to this set: {e}"))?;
    }

    Ok(set)
}

#[cfg(target_os = "macos")]
pub fn as_cpu_tag(v: &Yaml) -> anyhow::Result<CpuAffinity> {
    use anyhow::Context;

    let v =
        crate::value::as_nonzero_i32(v).context("cpu tag should be valid nonzero isize value")?;
    Ok(CpuAffinity::new(v))
}
