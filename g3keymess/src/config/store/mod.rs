/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::path::Path;

use anyhow::anyhow;
use tokio::sync::oneshot;
use yaml_rust::{Yaml, yaml};

use g3_macros::AnyConfig;
use g3_types::metrics::NodeName;
use g3_yaml::{HybridParser, YamlDocPosition};

mod local;
mod redis;

mod registry;
pub(crate) use registry::{clear, get_all};

const CONFIG_KEY_STORE_TYPE: &str = "type";

pub trait KeyStoreConfig {
    fn name(&self) -> &NodeName;
    async fn load_keys(&self) -> anyhow::Result<()>;
    fn spawn_subscriber(&self) -> anyhow::Result<Option<oneshot::Sender<()>>> {
        Ok(None)
    }
}

#[derive(Clone, AnyConfig)]
#[def_fn(name, &NodeName)]
#[def_async_fn(load_keys, anyhow::Result<()>)]
#[def_fn(spawn_subscriber, anyhow::Result<Option<oneshot::Sender<()>>>)]
pub enum AnyKeyStoreConfig {
    Local(local::LocalKeyStoreConfig),
    Redis(redis::RedisKeyStoreConfig),
}

pub(crate) fn load_all(v: &Yaml, conf_dir: &Path) -> anyhow::Result<()> {
    let parser = HybridParser::new(conf_dir, g3_daemon::opts::config_file_extension());
    parser.foreach_map(v, |map, position| {
        let store = load_store(map, position)?;
        if let Some(old_store) = registry::add(store) {
            Err(anyhow!(
                "key store with name {} already exists",
                old_store.name()
            ))
        } else {
            Ok(())
        }
    })?;
    Ok(())
}

fn load_store(
    map: &yaml::Hash,
    position: Option<YamlDocPosition>,
) -> anyhow::Result<AnyKeyStoreConfig> {
    let store_type = g3_yaml::hash_get_required_str(map, CONFIG_KEY_STORE_TYPE)?;
    match g3_yaml::key::normalize(store_type).as_str() {
        "local" => {
            let config = local::LocalKeyStoreConfig::parse(map, position)?;
            Ok(AnyKeyStoreConfig::Local(config))
        }
        "redis" => {
            let config = redis::RedisKeyStoreConfig::parse(map, position)?;
            Ok(AnyKeyStoreConfig::Redis(config))
        }
        _ => Err(anyhow!("unsupported key store type {store_type}")),
    }
}
