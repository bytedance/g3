/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::path::Path;

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_socket::BindAddr;

use super::HickoryDriverConfig;

impl HickoryDriverConfig {
    pub fn set_by_yaml_kv(
        &mut self,
        k: &str,
        v: &Yaml,
        lookup_dir: Option<&Path>,
    ) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            "server" => match v {
                Yaml::String(addrs) => self.parse_server_str(addrs),
                Yaml::Array(seq) => self.parse_server_array(seq),
                _ => Err(anyhow!("invalid yaml value type, expect string / array")),
            },
            "server_port" => {
                let port = g3_yaml::value::as_u16(v)?;
                self.server_port = Some(port);
                Ok(())
            }
            "encryption" | "encrypt" => {
                let config = g3_yaml::value::as_dns_encryption_protocol_builder(v, lookup_dir)
                    .context(format!("invalid dns encryption config value for key {k}"))?;
                self.encryption = Some(config);
                Ok(())
            }
            "connect_timeout" => {
                self.connect_timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "request_timeout" => {
                self.request_timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "each_timeout" => {
                self.each_timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                Ok(())
            }
            "each_tries" | "retry_attempts" => {
                self.each_tries = g3_yaml::value::as_i32(v)?;
                Ok(())
            }
            "bind_ip" => {
                let ip = g3_yaml::value::as_ipaddr(v)?;
                self.bind_addr = BindAddr::Ip(ip);
                Ok(())
            }
            #[cfg(any(
                target_os = "linux",
                target_os = "android",
                target_os = "macos",
                target_os = "illumos",
                target_os = "solaris"
            ))]
            "bind_interface" => {
                let interface = g3_yaml::value::as_interface(v)
                    .context(format!("invalid interface name value for key {k}"))?;
                self.bind_addr = BindAddr::Interface(interface);
                Ok(())
            }
            "tcp_misc_opts" => {
                self.tcp_misc_opts = g3_yaml::value::as_tcp_misc_sock_opts(v)
                    .context(format!("invalid tcp misc sock opts value for key {k}"))?;
                Ok(())
            }
            "udp_misc_opts" => {
                self.udp_misc_opts = g3_yaml::value::as_udp_misc_sock_opts(v)
                    .context(format!("invalid udp misc sock opts value for key {k}"))?;
                Ok(())
            }
            "positive_min_ttl" => {
                self.positive_min_ttl = g3_yaml::value::as_u32(v)?;
                Ok(())
            }
            "positive_max_ttl" => {
                self.positive_max_ttl = g3_yaml::value::as_u32(v)?;
                Ok(())
            }
            "negative_min_ttl" | "negative_ttl" => {
                self.negative_ttl = g3_yaml::value::as_u32(v)?;
                Ok(())
            }
            "negative_max_ttl" => Ok(()),
            _ => Err(anyhow!("invalid key {k}")),
        }
    }
}
