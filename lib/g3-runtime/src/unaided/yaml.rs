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

#[cfg(test)]
mod tests {
    use super::*;
    use g3_yaml::yaml_doc;
    use yaml_rust::YamlLoader;

    #[test]
    fn parse_yaml_ok() {
        // Full config except sched_affinity
        let yaml = yaml_doc!(
            r#"
            thread_number_total: 8
            thread_number_per_runtime: 2
            thread_stack_size: "2MB"
            max_io_events_per_tick: 512
        "#
        );
        let config = UnaidedRuntimeConfig::parse_yaml(&yaml).unwrap();
        assert_eq!(config.thread_number_total.get(), 8);
        assert_eq!(config.thread_number_per_rt.get(), 2);
        assert_eq!(config.thread_stack_size, Some(2 * 1000 * 1000));
        assert_eq!(config.max_io_events_per_tick, Some(512));

        // Sched_affinity
        #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "netbsd",
            windows,
        ))]
        {
            let yaml = yaml_doc!(
                r#"
                thread_number_total: 8
                sched_affinity:
                    0: "0-3"
                    1: "4-7"
            "#
            );
            let config = UnaidedRuntimeConfig::parse_yaml(&yaml).unwrap();
            assert_eq!(config.thread_number_total.get(), 8);
            assert_eq!(config.sched_affinity.len(), 2);

            let yaml = yaml_doc!(
                r#"
                thread_number_total: 4
                sched_affinity: true
            "#
            );
            let config = UnaidedRuntimeConfig::parse_yaml(&yaml).unwrap();
            assert_eq!(config.thread_number_total.get(), 4);
            assert!(!config.sched_affinity.is_empty());
        }

        // Openssl async job configs
        #[cfg(feature = "openssl-async-job")]
        {
            let yaml = yaml_doc!(
                r#"
                thread_number_total: 4
                openssl_async_job_init_size: 16
                openssl_async_job_max_size: 64
            "#
            );
            let config = UnaidedRuntimeConfig::parse_yaml(&yaml).unwrap();
            assert_eq!(config.openssl_async_job_init_size, 16);
            assert_eq!(config.openssl_async_job_max_size, 64);
        }

        // Environment variable based sched_affinity
        #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "netbsd",
            windows,
        ))]
        {
            unsafe {
                std::env::set_var("WORKER_0_CPU_LIST", "0,1,2");
            }
            let yaml = yaml_doc!(
                r#"
                thread_number_total: 4
                sched_affinity: true
            "#
            );
            let config = UnaidedRuntimeConfig::parse_yaml(&yaml).unwrap();
            assert!(config.sched_affinity.contains_key(&0));
            unsafe {
                std::env::remove_var("WORKER_0_CPU_LIST");
            }
        }
    }

    #[test]
    fn parse_yaml_err() {
        // Invalid root type
        let yaml = YamlLoader::load_from_str("invalid").unwrap();
        assert!(UnaidedRuntimeConfig::parse_yaml(&yaml[0]).is_err());

        // Unknown key
        let yaml = yaml_doc!(
            r#"
            invalid_key: 10
        "#
        );
        assert!(UnaidedRuntimeConfig::parse_yaml(&yaml).is_err());

        // Invalid value type
        let yaml = yaml_doc!(
            r#"
            thread_number_total: "invalid"
        "#
        );
        assert!(UnaidedRuntimeConfig::parse_yaml(&yaml).is_err());

        // Thread number not divisible
        let yaml = yaml_doc!(
            r#"
            thread_number_total: 5
            thread_number_per_runtime: 2
        "#
        );
        assert!(UnaidedRuntimeConfig::parse_yaml(&yaml).is_err());

        // Invalid sched_affinity format
        #[cfg(any(
            target_os = "linux",
            target_os = "android",
            target_os = "freebsd",
            target_os = "dragonfly",
            target_os = "netbsd",
            windows,
        ))]
        {
            let yaml = yaml_doc!(
                r#"
                sched_affinity: "invalid"
            "#
            );
            assert!(UnaidedRuntimeConfig::parse_yaml(&yaml).is_err());
        }

        // Invalid thread_stack_size format
        let yaml = yaml_doc!(
            r#"
            thread_stack_size: "invalid"
        "#
        );
        assert!(UnaidedRuntimeConfig::parse_yaml(&yaml).is_err());
    }
}
