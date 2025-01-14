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

use g3_types::metrics::NodeName;
use g3_yaml::{HybridParser, YamlDocPosition};

pub(crate) mod dummy_close;
#[cfg(feature = "quic")]
pub(crate) mod keyless_quic;
pub(crate) mod keyless_tcp;
pub(crate) mod stream_tcp;

mod registry;
pub(crate) use registry::{clear, get_all};

const CONFIG_KEY_BACKEND_TYPE: &str = "type";
const CONFIG_KEY_BACKEND_NAME: &str = "name";

pub(crate) enum BackendConfigDiffAction {
    NoAction,
    SpawnNew,
    Reload,
    #[allow(unused)]
    UpdateInPlace(u64),
}

pub(crate) trait BackendConfig {
    fn name(&self) -> &NodeName;
    fn position(&self) -> Option<YamlDocPosition>;
    fn backend_type(&self) -> &'static str;

    fn diff_action(&self, new: &AnyBackendConfig) -> BackendConfigDiffAction;
}

#[derive(Clone)]
pub(crate) enum AnyBackendConfig {
    DummyClose(dummy_close::DummyCloseBackendConfig),
    StreamTcp(stream_tcp::StreamTcpBackendConfig),
    KeylessTcp(keyless_tcp::KeylessTcpBackendConfig),
    #[cfg(feature = "quic")]
    KeylessQuic(keyless_quic::KeylessQuicBackendConfig),
}

macro_rules! impl_transparent0 {
    ($f:tt, $v:ty) => {
        pub(crate) fn $f(&self) -> $v {
            match self {
                AnyBackendConfig::DummyClose(s) => s.$f(),
                AnyBackendConfig::StreamTcp(s) => s.$f(),
                AnyBackendConfig::KeylessTcp(s) => s.$f(),
                #[cfg(feature = "quic")]
                AnyBackendConfig::KeylessQuic(s) => s.$f(),
            }
        }
    };
}

macro_rules! impl_transparent1 {
    ($f:tt, $v:ty, $p:ty) => {
        pub(crate) fn $f(&self, p: $p) -> $v {
            match self {
                AnyBackendConfig::DummyClose(s) => s.$f(p),
                AnyBackendConfig::StreamTcp(s) => s.$f(p),
                AnyBackendConfig::KeylessTcp(s) => s.$f(p),
                #[cfg(feature = "quic")]
                AnyBackendConfig::KeylessQuic(s) => s.$f(p),
            }
        }
    };
}

impl AnyBackendConfig {
    impl_transparent0!(name, &NodeName);
    impl_transparent0!(backend_type, &'static str);
    impl_transparent0!(position, Option<YamlDocPosition>);

    impl_transparent1!(diff_action, BackendConfigDiffAction, &Self);
}

pub(crate) fn load_all(v: &Yaml, conf_dir: &Path) -> anyhow::Result<()> {
    let parser = HybridParser::new(conf_dir, g3_daemon::opts::config_file_extension());
    parser.foreach_map(v, |map, position| {
        let backend = load_backend(map, position)?;
        registry::add(backend, false)?;
        Ok(())
    })?;
    Ok(())
}

pub(crate) fn load_at_position(position: &YamlDocPosition) -> anyhow::Result<AnyBackendConfig> {
    let doc = g3_yaml::load_doc(position)?;
    if let Yaml::Hash(map) = doc {
        let backend = load_backend(&map, Some(position.clone()))?;
        registry::add(backend.clone(), true)?;
        Ok(backend)
    } else {
        Err(anyhow!("yaml doc {position} is not a map"))
    }
}

fn load_backend(
    map: &yaml::Hash,
    position: Option<YamlDocPosition>,
) -> anyhow::Result<AnyBackendConfig> {
    let backend_type = g3_yaml::hash_get_required_str(map, CONFIG_KEY_BACKEND_TYPE)?;
    match g3_yaml::key::normalize(backend_type).as_str() {
        "dummy_close" | "dummyclose" => {
            let backend = dummy_close::DummyCloseBackendConfig::parse(map, position)
                .context("failed to load this DummyClose backend")?;
            Ok(AnyBackendConfig::DummyClose(backend))
        }
        "stream_tcp" | "streamtcp" => {
            let backend = stream_tcp::StreamTcpBackendConfig::parse(map, position)
                .context("failed to load this StreamTcp backend")?;
            Ok(AnyBackendConfig::StreamTcp(backend))
        }
        "keyless_tcp" | "keylesstcp" => {
            let backend = keyless_tcp::KeylessTcpBackendConfig::parse(map, position)
                .context("failed to load this KeylessTcp backend")?;
            Ok(AnyBackendConfig::KeylessTcp(backend))
        }
        #[cfg(feature = "quic")]
        "keyless_quic" | "keylessquic" => {
            let backend = keyless_quic::KeylessQuicBackendConfig::parse(map, position)
                .context("failed to load this KeylessQuic backend")?;
            Ok(AnyBackendConfig::KeylessQuic(backend))
        }
        _ => Err(anyhow!("unsupported backend type {}", backend_type)),
    }
}
