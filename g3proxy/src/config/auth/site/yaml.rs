/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use yaml_rust::Yaml;

use g3_yaml::YamlDocPosition;

use super::UserSiteConfig;

impl UserSiteConfig {
    pub(crate) fn parse_yaml(v: &Yaml, position: Option<&YamlDocPosition>) -> anyhow::Result<Self> {
        if let Yaml::Hash(map) = v {
            let mut config = UserSiteConfig::default();
            g3_yaml::foreach_kv(map, |k, v| config.set_yaml(k, v, position))?;
            config.check()?;
            Ok(config)
        } else {
            Err(anyhow!("yaml value type for 'user site' should be 'map'"))
        }
    }

    fn set_yaml(
        &mut self,
        k: &str,
        v: &Yaml,
        position: Option<&YamlDocPosition>,
    ) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            "id" | "name" => {
                self.id = g3_yaml::value::as_metric_node_name(v)
                    .context(format!("invalid metrics name value for key {k}"))?;
                Ok(())
            }
            "exact_match" => {
                let hosts = g3_yaml::value::as_list(v, g3_yaml::value::as_host)
                    .context(format!("invalid host list value for key {k}"))?;
                for host in hosts {
                    self.add_exact_host(host);
                }
                Ok(())
            }
            "subnet_match" => {
                let nets = g3_yaml::value::as_list(v, g3_yaml::value::as_ip_network)
                    .context(format!("invalid ip network list value for key {k}"))?;
                for net in nets {
                    self.subnet_match_ipaddr.insert(net);
                }
                Ok(())
            }
            "child_match" => {
                let domains = g3_yaml::value::as_list(v, g3_yaml::value::as_domain)
                    .context(format!("invalid domain list value for key {k}"))?;
                for domain in domains {
                    self.child_match_domain.insert(domain);
                }
                Ok(())
            }
            "emit_stats" | "emit_metrics" => {
                self.emit_stats = g3_yaml::value::as_bool(v)
                    .context(format!("invalid bool value for key {k}"))?;
                Ok(())
            }
            "duration_stats" | "duration_metrics" => {
                self.duration_stats = g3_yaml::value::as_histogram_metrics_config(v).context(
                    format!("invalid histogram metrics config value for key {k}"),
                )?;
                Ok(())
            }
            "resolve_strategy" => {
                let strategy = g3_yaml::value::as_resolve_strategy(v)
                    .context(format!("invalid resolve strategy value for key {k}"))?;
                self.resolve_strategy = Some(strategy);
                Ok(())
            }
            "tls_client" => {
                let lookup_dir = g3_daemon::config::get_lookup_dir(position)?;
                let builder = g3_yaml::value::as_to_many_openssl_tls_client_config_builder(
                    v,
                    Some(lookup_dir),
                )
                .context(format!("invalid tls client config value for key {k}"))?;
                self.tls_client = Some(builder);
                Ok(())
            }
            "http_rsp_header_recv_timeout" => {
                let timeout = g3_yaml::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                self.http_rsp_hdr_recv_timeout = Some(timeout);
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }
}
