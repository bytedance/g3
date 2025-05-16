/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::path::Path;

use anyhow::{Context, anyhow};
use yaml_rust::{Yaml, yaml};

use g3_macros::AnyConfig;
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
    fn r#type(&self) -> &'static str;

    fn diff_action(&self, new: &AnyBackendConfig) -> BackendConfigDiffAction;
}

#[derive(Clone, AnyConfig)]
#[def_fn(name, &NodeName)]
#[def_fn(r#type, &'static str)]
#[def_fn(position, Option<YamlDocPosition>)]
#[def_fn(diff_action, &Self, BackendConfigDiffAction)]
pub(crate) enum AnyBackendConfig {
    DummyClose(dummy_close::DummyCloseBackendConfig),
    StreamTcp(stream_tcp::StreamTcpBackendConfig),
    KeylessTcp(keyless_tcp::KeylessTcpBackendConfig),
    #[cfg(feature = "quic")]
    KeylessQuic(keyless_quic::KeylessQuicBackendConfig),
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
