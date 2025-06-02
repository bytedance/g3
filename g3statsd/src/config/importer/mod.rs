/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::path::Path;

use anyhow::{Context, anyhow};
use yaml_rust::{Yaml, yaml};

use g3_macros::AnyConfig;
use g3_types::metrics::NodeName;
use g3_yaml::{HybridParser, YamlDocPosition};

mod registry;
pub(crate) use registry::{clear, get_all};

pub(crate) mod dummy;
pub(crate) mod statsd;

const CONFIG_KEY_IMPORTER_TYPE: &str = "type";
const CONFIG_KEY_IMPORTER_NAME: &str = "name";

pub(crate) enum ImporterConfigDiffAction {
    NoAction,
    SpawnNew,
    ReloadNoRespawn,
    ReloadAndRespawn,
}

pub(crate) trait ImporterConfig {
    fn name(&self) -> &NodeName;
    fn position(&self) -> Option<YamlDocPosition>;
    fn importer_type(&self) -> &'static str;

    fn diff_action(&self, new: &AnyImporterConfig) -> ImporterConfigDiffAction;

    fn collector(&self) -> &NodeName;
}

#[derive(Clone, Debug, AnyConfig)]
#[def_fn(name, &NodeName)]
#[def_fn(position, Option<YamlDocPosition>)]
#[def_fn(importer_type, &'static str)]
#[def_fn(diff_action, &Self, ImporterConfigDiffAction)]
#[def_fn(collector, &NodeName)]
#[allow(clippy::large_enum_variant)]
pub(crate) enum AnyImporterConfig {
    Dummy(dummy::DummyImporterConfig),
    StatsDUdp(statsd::StatsdUdpImporterConfig),
    #[cfg(unix)]
    StatsDUnix(statsd::StatsdUnixImporterConfig),
}

pub(crate) fn load_all(v: &Yaml, conf_dir: &Path) -> anyhow::Result<()> {
    let parser = HybridParser::new(conf_dir, g3_daemon::opts::config_file_extension());
    parser.foreach_map(v, |map, position| {
        let importer = load_importer(map, position)?;
        if let Some(importer) = registry::add(importer) {
            Err(anyhow!(
                "importer with name {} already exists",
                importer.name()
            ))
        } else {
            Ok(())
        }
    })?;
    Ok(())
}

pub(crate) fn load_at_position(position: &YamlDocPosition) -> anyhow::Result<AnyImporterConfig> {
    let doc = g3_yaml::load_doc(position)?;
    if let Yaml::Hash(map) = doc {
        let importer = load_importer(&map, Some(position.clone()))?;
        registry::add(importer.clone());
        Ok(importer)
    } else {
        Err(anyhow!("yaml doc {position} is not a map"))
    }
}

fn load_importer(
    map: &yaml::Hash,
    position: Option<YamlDocPosition>,
) -> anyhow::Result<AnyImporterConfig> {
    let importer_type = g3_yaml::hash_get_required_str(map, CONFIG_KEY_IMPORTER_TYPE)?;
    match g3_yaml::key::normalize(importer_type).as_str() {
        "dummy" => {
            let importer = dummy::DummyImporterConfig::parse(map, position)
                .context("failed to load this Dummy importer")?;
            Ok(AnyImporterConfig::Dummy(importer))
        }
        "statsd" | "statsd_udp" => {
            let importer = statsd::StatsdUdpImporterConfig::parse(map, position)
                .context("failed to load this StatsD_UDP importer")?;
            Ok(AnyImporterConfig::StatsDUdp(importer))
        }
        #[cfg(unix)]
        "statsd_unix" => {
            let importer = statsd::StatsdUnixImporterConfig::parse(map, position)
                .context("failed to load this StatsD_UNIX importer")?;
            Ok(AnyImporterConfig::StatsDUnix(importer))
        }
        _ => Err(anyhow!("unsupported importer type {}", importer_type)),
    }
}
