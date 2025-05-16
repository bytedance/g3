/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use super::UnaidedRuntimeConfig;

impl UnaidedRuntimeConfig {
    pub fn parse_yaml(v: &Yaml) -> anyhow::Result<Self> {
        if let Yaml::Hash(map) = v {
            let mut config = UnaidedRuntimeConfig::default();
            #[cfg(any(
                target_os = "linux",
                target_os = "android",
                target_os = "freebsd",
                target_os = "dragonfly",
                target_os = "netbsd",
                windows,
            ))]
            let mut auto_set_sched_affinity = false;

            g3_yaml::foreach_kv(map, |k, v| match g3_yaml::key::normalize(k).as_str() {
                "thread_number_total" | "threads_total" | "thread_number" => {
                    let value = g3_yaml::value::as_nonzero_usize(v)?;
                    config.set_thread_number_total(value);
                    Ok(())
                }
                "thread_number_per_runtime" | "threads_per_runtime" => {
                    let value = g3_yaml::value::as_nonzero_usize(v)?;
                    config.set_thread_number_per_rt(value);
                    Ok(())
                }
                "thread_stack_size" => {
                    let value = g3_yaml::humanize::as_usize(v)
                        .context(format!("invalid humanize usize value for key {k}"))?;
                    config.set_thread_stack_size(value);
                    Ok(())
                }
                #[cfg(any(
                    target_os = "linux",
                    target_os = "android",
                    target_os = "freebsd",
                    target_os = "dragonfly",
                    target_os = "netbsd",
                    windows,
                ))]
                "sched_affinity" => {
                    if let Yaml::Hash(map) = v {
                        for (ik, iv) in map.iter() {
                            let id = g3_yaml::value::as_usize(ik)
                                .context(format!("the keys for {k} should be usize value"))?;

                            let cpu = g3_yaml::value::as_cpu_set(iv)
                                .context(format!("invalid cpu set value for {k}/{id}"))?;

                            config.set_sched_affinity(id, cpu);
                        }
                        Ok(())
                    } else if let Ok(enable) = g3_yaml::value::as_bool(v) {
                        auto_set_sched_affinity = enable;
                        Ok(())
                    } else {
                        Err(anyhow!("invalid map value for key {k}"))
                    }
                }
                "max_io_events_per_tick" => {
                    let capacity = g3_yaml::value::as_usize(v)?;
                    config.set_max_io_events_per_tick(capacity);
                    Ok(())
                }
                #[cfg(feature = "openssl-async-job")]
                "openssl_async_job_init_size" => {
                    let size = g3_yaml::value::as_usize(v)?;
                    config.set_openssl_async_job_init_size(size);
                    Ok(())
                }
                #[cfg(feature = "openssl-async-job")]
                "openssl_async_job_max_size" => {
                    let size = g3_yaml::value::as_usize(v)?;
                    config.set_openssl_async_job_init_size(size);
                    Ok(())
                }
                _ => Err(anyhow!("invalid key {k}")),
            })?;

            #[cfg(any(
                target_os = "linux",
                target_os = "android",
                target_os = "freebsd",
                target_os = "dragonfly",
                target_os = "netbsd",
                windows,
            ))]
            if auto_set_sched_affinity {
                config
                    .auto_set_sched_affinity()
                    .context("failed to set all mapped sched affinity")?;
            }

            config.check().context("invalid worker config")?;
            Ok(config)
        } else {
            Err(anyhow!(
                "yaml value type for 'unaided runtime config' should be 'map'"
            ))
        }
    }
}
