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

use std::path::Path;

use anyhow::anyhow;
use yaml_rust::{yaml, Yaml};

use g3_yaml::{HybridParser, YamlDocPosition};

mod registry;
pub(crate) use registry::{clear, get_all};

mod auditor;
pub(crate) use auditor::AuditorConfig;

pub(crate) fn load_all(v: &Yaml, conf_dir: &Path) -> anyhow::Result<()> {
    let parser = HybridParser::new(conf_dir, crate::config::config_file_extension());
    parser.foreach_map(v, &|map, position| {
        let auditor = load_auditor(map, position)?;
        registry::add(auditor, false)?;
        Ok(())
    })
}

pub(crate) fn load_at_position(position: &YamlDocPosition) -> anyhow::Result<AuditorConfig> {
    let doc = g3_yaml::load_doc(position)?;
    if let Yaml::Hash(map) = doc {
        let auditor = load_auditor(&map, Some(position.clone()))?;
        registry::add(auditor.clone(), true)?;
        Ok(auditor)
    } else {
        Err(anyhow!("yaml doc {position} is not a map"))
    }
}

fn load_auditor(
    map: &yaml::Hash,
    position: Option<YamlDocPosition>,
) -> anyhow::Result<AuditorConfig> {
    let mut auditor = AuditorConfig::new(position);
    auditor.parse(map)?;
    Ok(auditor)
}
