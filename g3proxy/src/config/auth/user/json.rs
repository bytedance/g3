/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use ahash::AHashMap;
use anyhow::{Context, anyhow};
use log::warn;
use serde_json::{Map, Value};

use g3_types::metrics::NodeName;

use super::{PasswordToken, UserConfig, UserSiteConfig};
use crate::escape::EgressPathSelection;

impl UserConfig {
    pub(crate) fn parse_json(map: &Map<String, Value>) -> anyhow::Result<Self> {
        let mut config = UserConfig::default();
        for (k, v) in map {
            config.set_json(k, v)?;
        }
        config.check()?;
        Ok(config)
    }

    fn set_json(&mut self, k: &str, v: &Value) -> anyhow::Result<()> {
        match g3_json::key::normalize(k).as_str() {
            "name" => {
                let name =
                    g3_json::value::as_string(v).context(format!("invalid value for key {k}"))?;
                self.name = name.into();
                Ok(())
            }
            "token" => {
                self.password_token =
                    PasswordToken::parse_json(v).context(format!("invalid value for key {k}"))?;
                Ok(())
            }
            "expire" => {
                let expire_datetime = g3_json::value::as_rfc3339_datetime(v)
                    .context(format!("invalid rfc3339 datetime value for key {k}"))?;
                self.expire_datetime = Some(expire_datetime);
                Ok(())
            }
            "block_and_delay" => {
                let delay = g3_json::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                self.block_and_delay = Some(delay);
                Ok(())
            }
            "tcp_connect" => {
                let config = g3_json::value::as_tcp_connect_config(v)
                    .context(format!("invalid tcp connect config value for key {k}"))?;
                self.tcp_connect = Some(config);
                Ok(())
            }
            "tcp_sock_speed_limit" => {
                self.tcp_sock_speed_limit = g3_json::value::as_tcp_sock_speed_limit(v)
                    .context(format!("invalid tcp socket speed limit value for key {k}"))?;
                Ok(())
            }
            "tcp_conn_speed_limit" | "tcp_conn_limit" => {
                warn!("deprecated config key {k}, please use 'tcp_sock_speed_limit' instead");
                self.set_json("tcp_sock_speed_limit", v)
            }
            "udp_sock_speed_limit" => {
                self.udp_sock_speed_limit = g3_json::value::as_udp_sock_speed_limit(v)
                    .context(format!("invalid udp socket speed limit value for key {k}"))?;
                Ok(())
            }
            "udp_relay_speed_limit" | "udp_relay_limit" => {
                warn!("deprecated config key {k}, please use 'udp_sock_speed_limit' instead");
                self.set_json("udp_sock_speed_limit", v)
            }
            "tcp_all_upload_speed_limit" => {
                let limit = g3_json::value::as_global_stream_speed_limit(v).context(format!(
                    "invalid global stream speed limit config value for key {k}"
                ))?;
                self.tcp_all_upload_speed_limit = Some(limit);
                Ok(())
            }
            "tcp_all_download_speed_limit" => {
                let limit = g3_json::value::as_global_stream_speed_limit(v).context(format!(
                    "invalid global stream speed limit config value for key {k}"
                ))?;
                self.tcp_all_download_speed_limit = Some(limit);
                Ok(())
            }
            "udp_all_upload_speed_limit" => {
                let limit = g3_json::value::as_global_datagram_speed_limit(v).context(format!(
                    "invalid global datagram speed limit config value for key {k}"
                ))?;
                self.udp_all_upload_speed_limit = Some(limit);
                Ok(())
            }
            "udp_all_download_speed_limit" => {
                let limit = g3_json::value::as_global_datagram_speed_limit(v).context(format!(
                    "invalid global datagram speed limit config value for key {k}"
                ))?;
                self.udp_all_download_speed_limit = Some(limit);
                Ok(())
            }
            "tcp_remote_keepalive" => {
                self.tcp_remote_keepalive = g3_json::value::as_tcp_keepalive_config(v)
                    .context(format!("invalid tcp keepalive config value for key {k}"))?;
                Ok(())
            }
            "tcp_remote_misc_opts" => {
                let opts = g3_json::value::as_tcp_misc_sock_opts(v)
                    .context(format!("invalid tcp misc sock opts value for key {k}"))?;
                self.tcp_remote_misc_opts = Some(opts);
                Ok(())
            }
            "udp_remote_misc_opts" => {
                let opts = g3_json::value::as_udp_misc_sock_opts(v)
                    .context(format!("invalid udp misc sock opts value for key {k}"))?;
                self.udp_remote_misc_opts = Some(opts);
                Ok(())
            }
            "tcp_client_misc_opts" => {
                let opts = g3_json::value::as_tcp_misc_sock_opts(v)
                    .context(format!("invalid tcp misc sock opts value for key {k}"))?;
                self.tcp_client_misc_opts = Some(opts);
                Ok(())
            }
            "udp_client_misc_opts" => {
                let opts = g3_json::value::as_udp_misc_sock_opts(v)
                    .context(format!("invalid udp misc sock opts value for key {k}"))?;
                self.udp_client_misc_opts = Some(opts);
                Ok(())
            }
            "http_upstream_keepalive" => {
                self.http_upstream_keepalive = g3_json::value::as_http_keepalive_config(v)
                    .context(format!("invalid http keepalive config value for key {k}"))?;
                Ok(())
            }
            "http_rsp_header_recv_timeout" => {
                let timeout = g3_json::humanize::as_duration(v)
                    .context(format!("invalid humanize duration value for key {k}"))?;
                self.http_rsp_hdr_recv_timeout = Some(timeout);
                Ok(())
            }
            "tcp_conn_rate_limit" | "tcp_conn_limit_quota" => {
                let quota = g3_json::value::as_rate_limit_quota(v)
                    .context(format!("invalid request quota value for key {k}"))?;
                self.tcp_conn_rate_limit = Some(quota);
                Ok(())
            }
            "request_rate_limit" | "request_limit_quota" => {
                let quota = g3_json::value::as_rate_limit_quota(v)
                    .context(format!("invalid request quota value for key {k}"))?;
                self.request_rate_limit = Some(quota);
                Ok(())
            }
            "request_max_alive" | "request_alive_max" => {
                self.request_alive_max = g3_json::value::as_usize(v)
                    .context(format!("invalid usize value for key {k}"))?;
                Ok(())
            }
            "ingress_network_filter" | "ingress_net_filter" => {
                let filter = g3_json::value::acl::as_ingress_network_rule_builder(v).context(
                    format!("invalid ingress network acl rule value for key {k}"),
                )?;
                self.ingress_net_filter = Some(filter);
                Ok(())
            }
            "proxy_request_filter" => {
                let filter = g3_json::value::acl::as_proxy_request_rule(v)
                    .context(format!("invalid proxy request acl rule value for key {k}"))?;
                self.proxy_request_filter = Some(filter);
                Ok(())
            }
            "dst_host_filter_set" => {
                let builder = g3_json::value::acl_set::as_dst_host_rule_set_builder(v)
                    .context(format!("invalid dst host acl rule value for key {k}"))?;
                self.dst_host_filter = Some(builder);
                Ok(())
            }
            "dst_port_filter" => {
                let filter = g3_json::value::acl::as_exact_port_rule(v)
                    .context(format!("invalid dst port acl rule value for key {k}"))?;
                self.dst_port_filter = Some(filter);
                Ok(())
            }
            "http_user_agent_filter" => {
                let filter = g3_json::value::acl::as_user_agent_rule(v)
                    .context(format!("invalid user agent acl rule value for key {k}"))?;
                self.http_user_agent_filter = Some(filter);
                Ok(())
            }
            "resolve_strategy" => {
                let strategy = g3_json::value::as_resolve_strategy(v)
                    .context(format!("invalid resolve strategy value for key {k}"))?;
                self.resolve_strategy = Some(strategy);
                Ok(())
            }
            "resolve_redirection" => {
                let builder = g3_json::value::as_resolve_redirection_builder(v)
                    .context(format!("invalid resolve redirection value for key {k}"))?;
                self.resolve_redirection = Some(builder);
                Ok(())
            }
            "log_rate_limit" | "log_limit_quota" => {
                let quota = g3_json::value::as_rate_limit_quota(v)
                    .context(format!("invalid request quota value for key {k}"))?;
                self.log_rate_limit = Some(quota);
                Ok(())
            }
            "log_uri_max_chars" | "uri_log_max_chars" => {
                let max_chars = g3_json::value::as_usize(v)
                    .context(format!("invalid usize value for key {k}"))?;
                self.log_uri_max_chars = Some(max_chars);
                Ok(())
            }
            "task_idle_max_count" => {
                let count = g3_json::value::as_usize(v)
                    .context(format!("invalid usize value for key {k}"))?;
                self.task_idle_max_count = Some(count);
                Ok(())
            }
            "socks_use_udp_associate" => {
                self.socks_use_udp_associate = g3_json::value::as_bool(v)
                    .context(format!("invalid bool value for key {k}"))?;
                Ok(())
            }
            "explicit_sites" => {
                if let Value::Array(seq) = v {
                    for (i, v) in seq.iter().enumerate() {
                        let site_group = UserSiteConfig::parse_json(v)
                            .context(format!("invalid user site group value for {k}#{i}"))?;
                        self.add_site_group(site_group)?;
                    }
                    Ok(())
                } else {
                    Err(anyhow!(
                        "invalid sequence of user site group value for key {k}"
                    ))
                }
            }
            "audit" => self
                .audit
                .parse_json(v)
                .context(format!("invalid user audit config value for key {k}")),
            "egress_path_id_map" => {
                let id_map = g3_json::value::as_hashmap(
                    v,
                    |v| NodeName::from_str(v).map_err(|e| anyhow!("invalid metrics name: {e}")),
                    g3_json::value::as_string,
                )
                .context(format!("invalid egress path id map value for key {k}"))?;
                self.egress_path_selection = Some(EgressPathSelection::MatchId(
                    id_map.into_iter().collect::<AHashMap<_, _>>(),
                ));
                Ok(())
            }
            "egress_path_value_map" => {
                let value_map = g3_json::value::as_hashmap(
                    v,
                    |v| NodeName::from_str(v).map_err(|e| anyhow!("invalid metrics name: {e}")),
                    |v| Ok(v.clone()),
                )
                .context(format!("invalid egress path value map value for key {k}"))?;
                self.egress_path_selection = Some(EgressPathSelection::MatchValue(
                    value_map.into_iter().collect::<AHashMap<_, _>>(),
                ));
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }
}
