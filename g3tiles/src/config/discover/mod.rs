/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use anyhow::{anyhow, Context};
use yaml_rust::{yaml, Yaml};

use g3_types::metrics::MetricsName;
use g3_yaml::{HybridParser, YamlDocPosition};

mod registry;
pub(crate) use registry::{clear, get_all};

pub(crate) mod host_resolver;
pub(crate) mod static_addr;

const CONFIG_KEY_DISCOVER_TYPE: &str = "type";
const CONFIG_KEY_DISCOVER_NAME: &str = "name";

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DiscoverRegisterData {
    Null,
    Yaml(Yaml),
    #[allow(unused)]
    Json(serde_json::Value),
}

pub(crate) enum DiscoverConfigDiffAction {
    NoAction,
    SpawnNew,
    #[allow(unused)]
    UpdateInPlace,
}

pub(crate) trait DiscoverConfig {
    fn name(&self) -> &MetricsName;
    fn position(&self) -> Option<YamlDocPosition>;
    fn discover_type(&self) -> &'static str;

    fn diff_action(&self, new: &AnyDiscoverConfig) -> DiscoverConfigDiffAction;
}

#[derive(Clone)]
pub(crate) enum AnyDiscoverConfig {
    StaticAddr(static_addr::StaticAddrDiscoverConfig),
    HostResolver(host_resolver::HostResolverDiscoverConfig),
}

macro_rules! impl_transparent0 {
    ($f:tt, $v:ty) => {
        pub(crate) fn $f(&self) -> $v {
            match self {
                AnyDiscoverConfig::StaticAddr(d) => d.$f(),
                AnyDiscoverConfig::HostResolver(d) => d.$f(),
            }
        }
    };
}

macro_rules! impl_transparent1 {
    ($f:tt, $v:ty, $p:ty) => {
        pub(crate) fn $f(&self, p: $p) -> $v {
            match self {
                AnyDiscoverConfig::StaticAddr(d) => d.$f(p),
                AnyDiscoverConfig::HostResolver(d) => d.$f(p),
            }
        }
    };
}

impl AnyDiscoverConfig {
    impl_transparent0!(name, &MetricsName);
    impl_transparent0!(position, Option<YamlDocPosition>);

    impl_transparent1!(diff_action, DiscoverConfigDiffAction, &Self);
}

pub(crate) fn load_all(v: &Yaml, conf_dir: &Path) -> anyhow::Result<()> {
    let parser = HybridParser::new(conf_dir, g3_daemon::opts::config_file_extension());
    parser.foreach_map(v, |map, position| {
        let site = load_discover(map, position)?;
        registry::add(site, false)?;
        Ok(())
    })?;
    Ok(())
}

pub(crate) fn load_at_position(position: &YamlDocPosition) -> anyhow::Result<AnyDiscoverConfig> {
    let doc = g3_yaml::load_doc(position)?;
    if let Yaml::Hash(map) = doc {
        let site = load_discover(&map, Some(position.clone()))?;
        registry::add(site.clone(), true)?;
        Ok(site)
    } else {
        Err(anyhow!("yaml doc {position} is not a map"))
    }
}

fn load_discover(
    map: &yaml::Hash,
    position: Option<YamlDocPosition>,
) -> anyhow::Result<AnyDiscoverConfig> {
    let discover_type = g3_yaml::hash_get_required_str(map, CONFIG_KEY_DISCOVER_TYPE)?;
    match g3_yaml::key::normalize(discover_type).as_str() {
        "static_addr" | "staticaddr" => {
            let discover = static_addr::StaticAddrDiscoverConfig::parse_yaml_conf(map, position)
                .context("failed to load this StaticAddr discover")?;
            Ok(AnyDiscoverConfig::StaticAddr(discover))
        }
        "host_resolver" | "hostresolver" => {
            let discover =
                host_resolver::HostResolverDiscoverConfig::parse_yaml_conf(map, position)
                    .context("failed to load this HostResolver discover")?;
            Ok(AnyDiscoverConfig::HostResolver(discover))
        }
        _ => Err(anyhow!("unsupported discover type {}", discover_type)),
    }
}
