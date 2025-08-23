/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::path::Path;

use anyhow::anyhow;
use yaml_rust::{Yaml, yaml};

use g3_yaml::{HybridParser, YamlDocPosition};

mod registry;
pub(crate) use registry::{clear, get_all};

mod auditor;
pub(crate) use auditor::AuditorConfig;

#[cfg(feature = "quic")]
mod detour;
#[cfg(feature = "quic")]
pub(crate) use detour::AuditStreamDetourConfig;

pub(crate) fn load_all(v: &Yaml, conf_dir: &Path) -> anyhow::Result<()> {
    let parser = HybridParser::new(conf_dir, g3_daemon::opts::config_file_extension());
    parser.foreach_map(v, |map, position| {
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
