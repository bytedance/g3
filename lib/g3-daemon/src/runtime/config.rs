/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::time::Duration;

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_runtime::blended::BlendedRuntimeConfig;
use g3_runtime::unaided::UnaidedRuntimeConfig;
use g3_types::sync::GlobalInit;

static RUNTIME_CONFIG: GlobalInit<BlendedRuntimeConfig> =
    GlobalInit::new(BlendedRuntimeConfig::new());
static WORKER_CONFIG: GlobalInit<Option<UnaidedRuntimeConfig>> = GlobalInit::new(None);
static GRACEFUL_WAIT_CONFIG: GlobalInit<GracefulWaitConfig> =
    GlobalInit::new(GracefulWaitConfig::new());

struct GracefulWaitConfig {
    server_offline_delay: Duration,
    task_wait_timeout: Duration,
    task_quit_timeout: Duration,
    task_wait_delay: Duration,
}

impl Default for GracefulWaitConfig {
    fn default() -> Self {
        Self::new()
    }
}

impl GracefulWaitConfig {
    const fn new() -> Self {
        GracefulWaitConfig {
            server_offline_delay: Duration::from_secs(4),
            task_wait_timeout: Duration::from_secs(36000),
            task_quit_timeout: Duration::from_secs(1800),
            task_wait_delay: Duration::from_secs(2),
        }
    }
}

pub fn get_runtime_config() -> &'static BlendedRuntimeConfig {
    RUNTIME_CONFIG.as_ref()
}

pub fn get_worker_config() -> Option<&'static UnaidedRuntimeConfig> {
    WORKER_CONFIG.as_ref().as_ref()
}

pub fn get_server_offline_delay() -> Duration {
    GRACEFUL_WAIT_CONFIG.as_ref().server_offline_delay
}

pub fn get_task_wait_delay() -> Duration {
    GRACEFUL_WAIT_CONFIG.as_ref().task_wait_delay
}

pub fn get_task_wait_timeout() -> Duration {
    GRACEFUL_WAIT_CONFIG.as_ref().task_wait_timeout
}

pub fn get_task_quit_timeout() -> Duration {
    GRACEFUL_WAIT_CONFIG.as_ref().task_quit_timeout
}

pub fn load(v: &Yaml) -> anyhow::Result<()> {
    match v {
        Yaml::Hash(map) => g3_yaml::foreach_kv(map, set_global_config),
        Yaml::Null => Ok(()),
        _ => Err(anyhow!("root value type should be hash")),
    }
}

pub fn load_worker(v: &Yaml) -> anyhow::Result<()> {
    let config = UnaidedRuntimeConfig::parse_yaml(v)?;
    WORKER_CONFIG.with_mut(|v| v.replace(config));
    Ok(())
}

pub fn set_default_thread_number(num: usize) {
    RUNTIME_CONFIG.with_mut(|config| config.set_default_thread_number(num));
}

fn set_global_config(k: &str, v: &Yaml) -> anyhow::Result<()> {
    match g3_yaml::key::normalize(k).as_str() {
        "server_offline_delay" => {
            let value = g3_yaml::humanize::as_duration(v)
                .context(format!("invalid humanize duration value for key {k}"))?;
            GRACEFUL_WAIT_CONFIG.with_mut(|config| config.server_offline_delay = value);
            Ok(())
        }
        "task_wait_delay" => {
            let value = g3_yaml::humanize::as_duration(v)
                .context(format!("invalid humanize duration value for key {k}"))?;
            GRACEFUL_WAIT_CONFIG.with_mut(|config| config.task_wait_delay = value);
            Ok(())
        }
        "task_wait_timeout" => {
            let value = g3_yaml::humanize::as_duration(v)
                .context(format!("invalid humanize duration value for key {k}"))?;
            GRACEFUL_WAIT_CONFIG.with_mut(|config| config.task_wait_timeout = value);
            Ok(())
        }
        "task_quit_timeout" => {
            let value = g3_yaml::humanize::as_duration(v)
                .context(format!("invalid humanize duration value for key {k}"))?;
            GRACEFUL_WAIT_CONFIG.with_mut(|config| config.task_quit_timeout = value);
            Ok(())
        }
        _ => RUNTIME_CONFIG.with_mut(|config| config.parse_by_yaml_kv(k, v)),
    }
}
