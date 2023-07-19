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
use async_trait::async_trait;
use openssl::pkey::{PKey, Private};
use tokio::sync::oneshot;
use yaml_rust::{yaml, Yaml};

use g3_types::metrics::MetricsName;
use g3_yaml::{HybridParser, YamlDocPosition};

mod local;
mod redis;

mod registry;
pub(crate) use registry::{clear, get_all};

const CONFIG_KEY_STORE_TYPE: &str = "type";

#[async_trait]
pub trait KeyStoreConfig {
    fn name(&self) -> &MetricsName;
    async fn load_certs(&self) -> anyhow::Result<Vec<PKey<Private>>>;
    fn spawn_subscriber(&self) -> anyhow::Result<Option<oneshot::Sender<()>>> {
        Ok(None)
    }
}

macro_rules! impl_transparent0 {
    ($f:tt, $v:ty) => {
        pub(crate) fn $f(&self) -> $v {
            match self {
                AnyKeyStoreConfig::Local(s) => s.$f(),
                AnyKeyStoreConfig::Redis(s) => s.$f(),
            }
        }
    };
}

macro_rules! impl_async_transparent0 {
    ($f:tt, $v:ty) => {
        pub(crate) async fn $f(&self) -> $v {
            match self {
                AnyKeyStoreConfig::Local(s) => s.$f().await,
                AnyKeyStoreConfig::Redis(s) => s.$f().await,
            }
        }
    };
}

#[derive(Clone)]
pub enum AnyKeyStoreConfig {
    Local(local::LocalKeyStoreConfig),
    Redis(redis::RedisKeyStoreConfig),
}

impl AnyKeyStoreConfig {
    impl_transparent0!(name, &MetricsName);
    impl_async_transparent0!(load_certs, anyhow::Result<Vec<PKey<Private>>>);
    impl_transparent0!(
        spawn_subscriber,
        anyhow::Result<Option<oneshot::Sender<()>>>
    );
}

pub(crate) fn load_all(v: &Yaml, conf_dir: &Path) -> anyhow::Result<()> {
    let parser = HybridParser::new(conf_dir, g3_daemon::opts::config_file_extension());
    parser.foreach_map(v, &|map, position| {
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
