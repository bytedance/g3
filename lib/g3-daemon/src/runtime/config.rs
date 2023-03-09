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

use std::time::Duration;

use anyhow::{anyhow, Context};
use yaml_rust::Yaml;

use g3_runtime::blended::BlendedRuntimeConfig;
use g3_runtime::unaided::UnaidedRuntimeConfig;

static mut RUNTIME_CONFIG: BlendedRuntimeConfig = BlendedRuntimeConfig::new();
static mut WORKER_CONFIG: Option<UnaidedRuntimeConfig> = None;
static mut SERVER_OFFLINE_DELAY_DURATION: Duration = Duration::from_secs(4);
static mut TASK_WAIT_TIMEOUT_DURATION: Duration = Duration::from_secs(36000); // 10h
static mut TASK_QUIT_TIMEOUT_DURATION: Duration = Duration::from_secs(1800); // 0.5h
static mut TASK_WAIT_DELAY_DURATION: Duration = Duration::from_secs(2);

pub fn get_runtime_config() -> &'static BlendedRuntimeConfig {
    unsafe { &RUNTIME_CONFIG }
}

pub fn get_worker_config() -> Option<&'static UnaidedRuntimeConfig> {
    unsafe { WORKER_CONFIG.as_ref() }
}

pub fn get_server_offline_delay() -> Duration {
    unsafe { SERVER_OFFLINE_DELAY_DURATION }
}

pub fn get_task_wait_delay() -> Duration {
    unsafe { TASK_WAIT_DELAY_DURATION }
}

pub fn get_task_wait_timeout() -> Duration {
    unsafe { TASK_WAIT_TIMEOUT_DURATION }
}

pub fn get_task_quit_timeout() -> Duration {
    unsafe { TASK_QUIT_TIMEOUT_DURATION }
}

pub fn load(v: &Yaml) -> anyhow::Result<()> {
    match v {
        Yaml::Hash(map) => g3_yaml::foreach_kv(map, set_global_config),
        Yaml::Null => Ok(()),
        _ => Err(anyhow!("root value type should be hash")),
    }
}

pub fn load_worker(v: &Yaml) -> anyhow::Result<()> {
    let config = g3_yaml::value::as_unaided_runtime_config(v)?;
    unsafe { WORKER_CONFIG = Some(config) }
    Ok(())
}

fn set_global_config(k: &str, v: &Yaml) -> anyhow::Result<()> {
    match g3_yaml::key::normalize(k).as_str() {
        "server_offline_delay" => {
            let value = g3_yaml::humanize::as_duration(v)
                .context(format!("invalid humanize duration value for key {k}"))?;
            unsafe {
                SERVER_OFFLINE_DELAY_DURATION = value;
            }
            Ok(())
        }
        "task_wait_delay" => {
            let value = g3_yaml::humanize::as_duration(v)
                .context(format!("invalid humanize duration value for key {k}"))?;
            unsafe {
                TASK_WAIT_DELAY_DURATION = value;
            }
            Ok(())
        }
        "task_wait_timeout" => {
            let value = g3_yaml::humanize::as_duration(v)
                .context(format!("invalid humanize duration value for key {k}"))?;
            unsafe {
                TASK_WAIT_TIMEOUT_DURATION = value;
            }
            Ok(())
        }
        "task_quit_timeout" => {
            let value = g3_yaml::humanize::as_duration(v)
                .context(format!("invalid humanize duration value for key {k}"))?;
            unsafe {
                TASK_QUIT_TIMEOUT_DURATION = value;
            }
            Ok(())
        }
        "thread_number" => {
            let value = g3_yaml::value::as_usize(v)?;
            unsafe {
                RUNTIME_CONFIG.set_thread_number(value);
            }
            Ok(())
        }
        "thread_name" => {
            let name = g3_yaml::value::as_ascii(v)
                .context(format!("invalid ascii string value for key {k}"))?;
            unsafe {
                RUNTIME_CONFIG.set_thread_name(name.as_str());
            }
            Ok(())
        }
        "thread_stack_size" => {
            let value = g3_yaml::humanize::as_usize(v)
                .context(format!("invalid humanize usize value for key {k}"))?;
            unsafe {
                RUNTIME_CONFIG.set_thread_stack_size(value);
            }
            Ok(())
        }
        "max_io_events_per_tick" => {
            let capacity = g3_yaml::value::as_usize(v)?;
            unsafe {
                RUNTIME_CONFIG.set_max_io_events_per_tick(capacity);
            }
            Ok(())
        }
        _ => Err(anyhow!("invalid key {k}")),
    }
}
