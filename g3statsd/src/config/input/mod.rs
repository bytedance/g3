/*
 * Copyright 2025 ByteDance and/or its affiliates.
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

use anyhow::{Context, anyhow};
use yaml_rust::{Yaml, yaml};

use g3_macros::AnyConfig;
use g3_types::metrics::NodeName;
use g3_yaml::{HybridParser, YamlDocPosition};

mod registry;
pub(crate) use registry::{clear, get_all};

pub(crate) mod dummy;
pub(crate) mod statsd;

const CONFIG_KEY_INPUT_TYPE: &str = "type";
const CONFIG_KEY_INPUT_NAME: &str = "name";

pub(crate) enum InputConfigDiffAction {
    NoAction,
    SpawnNew,
    ReloadOnlyConfig,
    ReloadAndRespawn,
}

pub(crate) trait InputConfig {
    fn name(&self) -> &NodeName;
    fn position(&self) -> Option<YamlDocPosition>;
    fn input_type(&self) -> &'static str;

    fn diff_action(&self, new: &AnyInputConfig) -> InputConfigDiffAction;
}

#[derive(Clone, Debug, AnyConfig)]
#[def_fn(name, &NodeName)]
#[def_fn(position, Option<YamlDocPosition>)]
#[def_fn(input_type, &'static str)]
#[def_fn(diff_action, &Self, InputConfigDiffAction)]
pub(crate) enum AnyInputConfig {
    Dummy(dummy::DummyInputConfig),
    StatsD(statsd::StatsdInputConfig),
}

pub(crate) fn load_all(v: &Yaml, conf_dir: &Path) -> anyhow::Result<()> {
    let parser = HybridParser::new(conf_dir, g3_daemon::opts::config_file_extension());
    parser.foreach_map(v, |map, position| {
        let input = load_input(map, position)?;
        if let Some(old_input) = registry::add(input) {
            Err(anyhow!(
                "input with name {} already exists",
                old_input.name()
            ))
        } else {
            Ok(())
        }
    })?;
    Ok(())
}

pub(crate) fn load_at_position(position: &YamlDocPosition) -> anyhow::Result<AnyInputConfig> {
    let doc = g3_yaml::load_doc(position)?;
    if let Yaml::Hash(map) = doc {
        let input = load_input(&map, Some(position.clone()))?;
        registry::add(input.clone());
        Ok(input)
    } else {
        Err(anyhow!("yaml doc {position} is not a map"))
    }
}

fn load_input(
    map: &yaml::Hash,
    position: Option<YamlDocPosition>,
) -> anyhow::Result<AnyInputConfig> {
    let input_type = g3_yaml::hash_get_required_str(map, CONFIG_KEY_INPUT_TYPE)?;
    match g3_yaml::key::normalize(input_type).as_str() {
        "dummy" => {
            let input = dummy::DummyInputConfig::parse(map, position)
                .context("failed to load this Dummy input")?;
            Ok(AnyInputConfig::Dummy(input))
        }
        "statsd" => {
            let input = statsd::StatsdInputConfig::parse(map, position)
                .context("failed to load this StatsD input")?;
            Ok(AnyInputConfig::StatsD(input))
        }
        _ => Err(anyhow!("unsupported input type {}", input_type)),
    }
}
