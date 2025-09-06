/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::path::Path;

use anyhow::anyhow;
use yaml_rust::{Yaml, yaml};

mod graphviz;
pub use graphviz::graphviz_graph;

mod mermaid;
pub use mermaid::mermaid_graph;

mod plantuml;
pub use plantuml::plantuml_graph;

pub(crate) mod audit;
pub(crate) mod auth;
pub(crate) mod escaper;
pub(crate) mod log;
pub(crate) mod resolver;
pub(crate) mod server;
pub(crate) mod sticky;

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

fn clear_all() {
    escaper::clear();
    audit::clear();
    auth::clear();
    server::clear();
    resolver::clear();
}

pub(crate) async fn reload() -> anyhow::Result<()> {
    tokio::task::spawn_blocking(reload_blocking)
        .await
        .map_err(|e| anyhow!("failed to join reload task: {e}"))?
}

fn reload_blocking() -> anyhow::Result<()> {
    clear_all();
    if let Some(conf_file) = g3_daemon::opts::config_file() {
        // allow multiple docs, and treat them as the same
        g3_yaml::foreach_doc(conf_file, |_, doc| match doc {
            Yaml::Hash(map) => reload_doc(map),
            _ => Err(anyhow!("yaml doc root should be hash")),
        })?;
    }
    Ok(())
}

fn reload_doc(map: &yaml::Hash) -> anyhow::Result<()> {
    let conf_dir =
        g3_daemon::opts::config_dir().ok_or_else(|| anyhow!("no valid config dir has been set"))?;
    g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
        "runtime" | "worker" | "log" | "stat" | "controller" => Ok(()),
        "sticky" => sticky::load(v, conf_dir),
        "escaper" => escaper::load_all(v, conf_dir),
        "server" => server::load_all(v, conf_dir),
        "resolver" => resolver::load_all(v, conf_dir),
        "user" | "user_group" => auth::load_all(v, conf_dir),
        "auditor" => audit::load_all(v, conf_dir),
        _ => Ok(()),
    })?;
    Ok(())
}

fn load_doc(map: &yaml::Hash) -> anyhow::Result<()> {
    let conf_dir =
        g3_daemon::opts::config_dir().ok_or_else(|| anyhow!("no valid config dir has been set"))?;
    g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
        "runtime" => g3_daemon::runtime::config::load(v),
        "worker" => g3_daemon::runtime::config::load_worker(v),
        "log" => log::load(v, conf_dir),
        "stat" => g3_daemon::stat::config::load(v, crate::build::PKG_NAME),
        "controller" => g3_daemon::control::config::load(v),
        "sticky" => sticky::load(v, conf_dir),
        "escaper" => escaper::load_all(v, conf_dir),
        "server" => server::load_all(v, conf_dir),
        "resolver" => resolver::load_all(v, conf_dir),
        "user" | "user_group" => auth::load_all(v, conf_dir),
        "auditor" => audit::load_all(v, conf_dir),
        _ => Err(anyhow!("invalid key {k} in main conf")),
    })?;
    Ok(())
}
