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

use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

use anyhow::anyhow;
use yaml_rust::{yaml, Yaml};

use g3_yaml::YamlDocPosition;

use crate::opts::ProcArgs;

pub(crate) mod log;

pub(crate) mod server;

static mut CONFIG_FILE_PATH: Option<PathBuf> = None;
static mut CONFIG_DIR_PATH: Option<PathBuf> = None;

static mut DAEMON_GROUP_NAME: String = String::new();
static mut CONFIG_FILE_EXTENSION: Option<OsString> = None;

pub(crate) fn config_dir() -> PathBuf {
    if let Some(dir) = unsafe { CONFIG_DIR_PATH.clone() } {
        dir
    } else {
        unreachable!()
    }
}

pub(crate) fn get_lookup_dir(position: Option<&YamlDocPosition>) -> PathBuf {
    if let Some(position) = position {
        if let Some(dir) = position.path.parent() {
            return dir.to_path_buf();
        }
    }
    config_dir()
}

pub(crate) fn daemon_group_name() -> &'static str {
    unsafe { &DAEMON_GROUP_NAME }
}

pub(crate) fn config_file_extension() -> Option<&'static OsStr> {
    unsafe { CONFIG_FILE_EXTENSION.as_deref() }
}

pub fn load(args: &ProcArgs) -> anyhow::Result<()> {
    let current_dir = std::env::current_dir()?;
    if let Some(ext) = args.config_file.extension() {
        unsafe { CONFIG_FILE_EXTENSION = Some(ext.to_os_string()) }
    }
    let conf_dir = args.config_file.parent().unwrap_or(&current_dir);

    unsafe {
        CONFIG_FILE_PATH = Some(args.config_file.clone());
        CONFIG_DIR_PATH = Some(PathBuf::from(conf_dir));
    }

    // allow multiple docs, and treat them as the same
    g3_yaml::foreach_doc(args.config_file.as_path(), |_, doc| match doc {
        Yaml::Hash(map) => load_doc(map, conf_dir),
        _ => Err(anyhow!("yaml doc root should be hash")),
    })?;

    if !args.group_name.is_empty() {
        unsafe { DAEMON_GROUP_NAME.clone_from(&args.group_name) }
    }

    Ok(())
}

fn clear_all() {
    server::clear();
}

pub(crate) async fn reload() -> anyhow::Result<()> {
    tokio::task::spawn_blocking(reload_blocking)
        .await
        .map_err(|e| anyhow!("failed to join reload task: {e}"))?
}

fn reload_blocking() -> anyhow::Result<()> {
    clear_all();
    if let (Some(conf_dir), Some(conf_file)) = unsafe { (&CONFIG_DIR_PATH, &CONFIG_FILE_PATH) } {
        // allow multiple docs, and treat them as the same
        g3_yaml::foreach_doc(conf_file.as_path(), |_, doc| match doc {
            Yaml::Hash(map) => reload_doc(map, conf_dir),
            _ => Err(anyhow!("yaml doc root should be hash")),
        })?;
    }
    Ok(())
}

fn reload_doc(map: &yaml::Hash, conf_dir: &Path) -> anyhow::Result<()> {
    g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
        "group_name" | "runtime" | "worker" | "log" | "stat" | "controller" => Ok(()),
        "server" => server::load_all(v, conf_dir),
        _ => Ok(()),
    })?;
    Ok(())
}

fn load_doc(map: &yaml::Hash, conf_dir: &Path) -> anyhow::Result<()> {
    g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
        "group_name" => match v {
            Yaml::String(name) => {
                unsafe { DAEMON_GROUP_NAME.clone_from(name) };
                Ok(())
            }
            _ => Err(anyhow!("invalid value for key {k}")),
        },
        "runtime" => g3_daemon::runtime::config::load(v),
        "worker" => g3_daemon::runtime::config::load_worker(v),
        "log" => log::load(v, conf_dir),
        "stat" => g3_daemon::stat::config::load(v, crate::build::PKG_NAME),
        "controller" => g3_daemon::control::config::load(v),
        "server" => server::load_all(v, conf_dir),
        _ => Err(anyhow!("invalid key {k} in main conf")),
    })?;
    Ok(())
}
