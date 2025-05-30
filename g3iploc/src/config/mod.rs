/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::path::Path;

use anyhow::anyhow;
use yaml_rust::{Yaml, yaml};

mod geoip;

pub fn load() -> anyhow::Result<&'static Path> {
    let config_file =
        g3_daemon::opts::config_file().ok_or_else(|| anyhow!("no config file set"))?;

    // allow multiple docs, and treat them as the same
    g3_yaml::foreach_doc(config_file, |_, doc| match doc {
        Yaml::Hash(map) => load_doc(map),
        _ => Err(anyhow!("yaml doc root should be hash")),
    })?;

    Ok(config_file)
}

fn load_doc(map: &yaml::Hash) -> anyhow::Result<()> {
    let conf_dir =
        g3_daemon::opts::config_dir().ok_or_else(|| anyhow!("no valid config dir has been set"))?;
    g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
        "runtime" => g3_daemon::runtime::config::load(v),
        "worker" => g3_daemon::runtime::config::load_worker(v),
        "stat" => g3_daemon::stat::config::load(v, crate::build::PKG_NAME),
        "geoip_db" => geoip::load(v, conf_dir),
        _ => Err(anyhow!("invalid key {k} in main conf")),
    })?;
    Ok(())
}
