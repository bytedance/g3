/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::{Context, anyhow};
use rand::distr::Bernoulli;
use yaml_rust::{Yaml, yaml};

use g3_cert_agent::CertAgentConfig;
use g3_dpi::{
    H1InterceptionConfig, H2InterceptionConfig, ImapInterceptionConfig,
    ProtocolInspectPolicyBuilder, ProtocolInspectionConfig, ProtocolPortMap,
    SmtpInterceptionConfig,
};
use g3_icap_client::IcapServiceConfig;
use g3_tls_ticket::TlsTicketConfig;
use g3_types::metrics::NodeName;
use g3_types::net::{
    OpensslInterceptionClientConfigBuilder, OpensslInterceptionServerConfigBuilder,
};
use g3_udpdump::StreamDumpConfig;
use g3_yaml::YamlDocPosition;

#[cfg(feature = "quic")]
use super::AuditStreamDetourConfig;

#[derive(Clone)]
pub(crate) struct AuditorConfig {
    name: NodeName,
    position: Option<YamlDocPosition>,
    pub(crate) protocol_inspection: ProtocolInspectionConfig,
    pub(crate) server_tcp_portmap: ProtocolPortMap,
    pub(crate) client_tcp_portmap: ProtocolPortMap,
    pub(crate) tls_cert_agent: Option<CertAgentConfig>,
    pub(crate) tls_ticketer: Option<TlsTicketConfig>,
    pub(crate) tls_interception_client: OpensslInterceptionClientConfigBuilder,
    pub(crate) tls_interception_server: OpensslInterceptionServerConfigBuilder,
    pub(crate) tls_stream_dump: Option<StreamDumpConfig>,
    pub(crate) log_uri_max_chars: usize,
    pub(crate) h1_interception: H1InterceptionConfig,
    pub(crate) h2_inspect_policy: ProtocolInspectPolicyBuilder,
    pub(crate) h2_interception: H2InterceptionConfig,
    pub(crate) websocket_inspect_policy: ProtocolInspectPolicyBuilder,
    pub(crate) smtp_inspect_policy: ProtocolInspectPolicyBuilder,
    pub(crate) smtp_interception: SmtpInterceptionConfig,
    pub(crate) imap_inspect_policy: ProtocolInspectPolicyBuilder,
    pub(crate) imap_interception: ImapInterceptionConfig,
    pub(crate) icap_reqmod_service: Option<Arc<IcapServiceConfig>>,
    pub(crate) icap_respmod_service: Option<Arc<IcapServiceConfig>>,
    #[cfg(feature = "quic")]
    pub(crate) stream_detour_service: Option<Arc<AuditStreamDetourConfig>>,
    pub(crate) task_audit_ratio: Bernoulli,
}

impl AuditorConfig {
    pub(crate) fn name(&self) -> &NodeName {
        &self.name
    }

    pub(crate) fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn with_name(name: NodeName, position: Option<YamlDocPosition>) -> Self {
        AuditorConfig {
            name,
            position,
            protocol_inspection: Default::default(),
            server_tcp_portmap: ProtocolPortMap::tcp_server(),
            client_tcp_portmap: ProtocolPortMap::tcp_client(),
            tls_cert_agent: None,
            tls_ticketer: None,
            tls_interception_client: Default::default(),
            tls_interception_server: Default::default(),
            tls_stream_dump: None,
            log_uri_max_chars: 1024,
            h1_interception: Default::default(),
            h2_inspect_policy: Default::default(),
            h2_interception: Default::default(),
            websocket_inspect_policy: Default::default(),
            smtp_inspect_policy: Default::default(),
            smtp_interception: Default::default(),
            imap_inspect_policy: Default::default(),
            imap_interception: Default::default(),
            icap_reqmod_service: None,
            icap_respmod_service: None,
            #[cfg(feature = "quic")]
            stream_detour_service: None,
            task_audit_ratio: Bernoulli::new(1.0).unwrap(),
        }
    }

    pub(crate) fn empty(name: &NodeName) -> Self {
        AuditorConfig::with_name(name.clone(), None)
    }

    pub(crate) fn new(position: Option<YamlDocPosition>) -> Self {
        AuditorConfig::with_name(NodeName::default(), position)
    }

    pub(crate) fn parse(&mut self, map: &yaml::Hash) -> anyhow::Result<()> {
        g3_yaml::foreach_kv(map, |k, v| self.set(k, v))?;
        self.check()?;
        Ok(())
    }

    fn check(&self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("name is not set"));
        }

        Ok(())
    }

    fn set(&mut self, k: &str, v: &Yaml) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(k).as_str() {
            "name" => {
                self.name = g3_yaml::value::as_metric_node_name(v)?;
                Ok(())
            }
            "protocol_inspection" => {
                let protocol_inspection = g3_yaml::value::as_protocol_inspection_config(v)
                    .context(format!(
                        "invalid protocol inspection config value for key {k}"
                    ))?;
                self.protocol_inspection = protocol_inspection;
                Ok(())
            }
            "server_tcp_portmap" => {
                g3_yaml::value::update_protocol_portmap(&mut self.server_tcp_portmap, v)
                    .context(format!("invalid protocol portmap value for key {k}"))
            }
            "client_tcp_portmap" => {
                g3_yaml::value::update_protocol_portmap(&mut self.client_tcp_portmap, v)
                    .context(format!("invalid protocol portmap value for key {k}"))
            }
            "tls_cert_agent" | "tls_cert_generator" => {
                let agent = CertAgentConfig::parse_yaml(v).context(format!(
                    "invalid tls cert generator config value for key {k}"
                ))?;
                self.tls_cert_agent = Some(agent);
                Ok(())
            }
            "tls_ticketer" => {
                let lookup_dir = g3_daemon::config::get_lookup_dir(self.position.as_ref())?;
                let ticketer = TlsTicketConfig::parse_yaml(v, Some(lookup_dir))
                    .context(format!("invalid tls ticket config value for key {k}"))?;
                self.tls_ticketer = Some(ticketer);
                Ok(())
            }
            "tls_interception_client" => {
                let lookup_dir = g3_daemon::config::get_lookup_dir(self.position.as_ref())?;
                let builder =
                    g3_yaml::value::as_tls_interception_client_config_builder(v, Some(lookup_dir))
                        .context(format!(
                            "invalid tls interception client config value for key {k}"
                        ))?;
                self.tls_interception_client = builder;
                Ok(())
            }
            "tls_interception_server" => {
                let builder = g3_yaml::value::as_tls_interception_server_config_builder(v)
                    .context(format!(
                        "invalid tls interception server config value for key {k}"
                    ))?;
                self.tls_interception_server = builder;
                Ok(())
            }
            "tls_stream_dump" => {
                let dump = StreamDumpConfig::parse_yaml(v)
                    .context(format!("invalid udp stream dump config value for key {k}"))?;
                self.tls_stream_dump = Some(dump);
                Ok(())
            }
            "log_uri_max_chars" | "uri_log_max_chars" => {
                self.log_uri_max_chars = g3_yaml::value::as_usize(v)
                    .context(format!("invalid usize value for key {k}"))?;
                Ok(())
            }
            "h1_interception" => {
                self.h1_interception = g3_yaml::value::as_h1_interception_config(v)
                    .context(format!("invalid h1 interception value for key {k}"))?;
                Ok(())
            }
            "h2_inspect_policy" => {
                self.h2_inspect_policy = g3_yaml::value::as_protocol_inspect_policy_builder(v)
                    .context(format!("invalid protocol inspect policy value for key {k}"))?;
                Ok(())
            }
            "h2_interception" => {
                self.h2_interception = g3_yaml::value::as_h2_interception_config(v)
                    .context(format!("invalid h1 interception value for key {k}"))?;
                Ok(())
            }
            "websocket_inspect_policy" => {
                self.websocket_inspect_policy =
                    g3_yaml::value::as_protocol_inspect_policy_builder(v)
                        .context(format!("invalid protocol inspect policy value for key {k}"))?;
                Ok(())
            }
            "smtp_inspect_policy" => {
                self.smtp_inspect_policy = g3_yaml::value::as_protocol_inspect_policy_builder(v)
                    .context(format!("invalid protocol inspect policy value for key {k}"))?;
                Ok(())
            }
            "smtp_interception" => {
                self.smtp_interception = g3_yaml::value::as_smtp_interception_config(v)
                    .context(format!("invalid smtp interception value for key {k}"))?;
                Ok(())
            }
            "imap_inspect_policy" => {
                self.imap_inspect_policy = g3_yaml::value::as_protocol_inspect_policy_builder(v)
                    .context(format!("invalid protocol inspect policy value for key {k}"))?;
                Ok(())
            }
            "imap_interception" => {
                self.imap_interception = g3_yaml::value::as_imap_interception_config(v)
                    .context(format!("invalid imap interception value for key {k}"))?;
                Ok(())
            }
            "icap_reqmod_service" => {
                let lookup_dir = g3_daemon::config::get_lookup_dir(self.position.as_ref())?;
                let service = IcapServiceConfig::parse_reqmod_service_yaml(v, Some(lookup_dir))
                    .context(format!(
                        "invalid icap reqmod service config value for key {k}"
                    ))?;
                self.icap_reqmod_service = Some(Arc::new(service));
                Ok(())
            }
            "icap_respmod_service" => {
                let lookup_dir = g3_daemon::config::get_lookup_dir(self.position.as_ref())?;
                let service = IcapServiceConfig::parse_respmod_service_yaml(v, Some(lookup_dir))
                    .context(format!(
                        "invalid icap respmod service config value for key {k}"
                    ))?;
                self.icap_respmod_service = Some(Arc::new(service));
                Ok(())
            }
            #[cfg(feature = "quic")]
            "stream_detour_service" => {
                let service = AuditStreamDetourConfig::parse(v, self.position.as_ref()).context(
                    format!("invalid audit stream detour config value for key {k}"),
                )?;
                self.stream_detour_service = Some(Arc::new(service));
                Ok(())
            }
            "task_audit_ratio" | "application_audit_ratio" => {
                self.task_audit_ratio = g3_yaml::value::as_random_ratio(v)
                    .context(format!("invalid random ratio value for key {k}"))?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {k}")),
        }
    }
}
