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

use std::sync::Arc;

use anyhow::{anyhow, Context};
use rand::distributions::Bernoulli;
use yaml_rust::{yaml, Yaml};

use g3_dpi::{
    H1InterceptionConfig, H2InterceptionConfig, ProtocolInspectPolicy, ProtocolInspectionConfig,
    ProtocolPortMap, SmtpInterceptionConfig,
};
use g3_icap_client::IcapServiceConfig;
use g3_tls_cert::agent::CertAgentConfig;
use g3_types::metrics::MetricsName;
use g3_types::net::{
    OpensslInterceptionClientConfigBuilder, OpensslInterceptionServerConfigBuilder,
};
use g3_udpdump::StreamDumpConfig;
use g3_yaml::YamlDocPosition;

#[derive(Clone)]
pub(crate) struct AuditorConfig {
    name: MetricsName,
    position: Option<YamlDocPosition>,
    pub(crate) protocol_inspection: ProtocolInspectionConfig,
    pub(crate) server_tcp_portmap: ProtocolPortMap,
    pub(crate) client_tcp_portmap: ProtocolPortMap,
    pub(crate) tls_cert_agent: Option<CertAgentConfig>,
    pub(crate) tls_interception_client: OpensslInterceptionClientConfigBuilder,
    pub(crate) tls_interception_server: OpensslInterceptionServerConfigBuilder,
    pub(crate) tls_stream_dump: Option<StreamDumpConfig>,
    pub(crate) log_uri_max_chars: usize,
    pub(crate) h1_interception: H1InterceptionConfig,
    pub(crate) h2_inspect_policy: ProtocolInspectPolicy,
    pub(crate) h2_interception: H2InterceptionConfig,
    pub(crate) smtp_inspect_policy: ProtocolInspectPolicy,
    pub(crate) smtp_interception: SmtpInterceptionConfig,
    pub(crate) icap_reqmod_service: Option<Arc<IcapServiceConfig>>,
    pub(crate) icap_respmod_service: Option<Arc<IcapServiceConfig>>,
    pub(crate) task_audit_ratio: Bernoulli,
}

impl AuditorConfig {
    pub(crate) fn name(&self) -> &MetricsName {
        &self.name
    }

    pub(crate) fn position(&self) -> Option<YamlDocPosition> {
        self.position.clone()
    }

    fn with_name(name: MetricsName, position: Option<YamlDocPosition>) -> Self {
        AuditorConfig {
            name,
            position,
            protocol_inspection: Default::default(),
            server_tcp_portmap: ProtocolPortMap::tcp_server(),
            client_tcp_portmap: ProtocolPortMap::tcp_client(),
            tls_cert_agent: None,
            tls_interception_client: Default::default(),
            tls_interception_server: Default::default(),
            tls_stream_dump: None,
            log_uri_max_chars: 1024,
            h1_interception: Default::default(),
            h2_inspect_policy: ProtocolInspectPolicy::Intercept,
            h2_interception: Default::default(),
            smtp_inspect_policy: ProtocolInspectPolicy::Intercept,
            smtp_interception: Default::default(),
            icap_reqmod_service: None,
            icap_respmod_service: None,
            task_audit_ratio: Bernoulli::new(1.0).unwrap(),
        }
    }

    pub(crate) fn empty(name: &MetricsName) -> Self {
        AuditorConfig::with_name(name.clone(), None)
    }

    pub(crate) fn new(position: Option<YamlDocPosition>) -> Self {
        AuditorConfig::with_name(MetricsName::default(), position)
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
                self.name = g3_yaml::value::as_metrics_name(v)?;
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
                let agent = g3_yaml::value::as_tls_cert_agent_config(v).context(format!(
                    "invalid tls cert generator config value for key {k}"
                ))?;
                self.tls_cert_agent = Some(agent);
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
                let dump = g3_yaml::value::as_stream_dump_config(v)
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
                self.h2_inspect_policy = g3_yaml::value::as_protocol_inspect_policy(v)
                    .context(format!("invalid protocol inspect policy value for key {k}"))?;
                Ok(())
            }
            "h2_interception" => {
                self.h2_interception = g3_yaml::value::as_h2_interception_config(v)
                    .context(format!("invalid h1 interception value for key {k}"))?;
                Ok(())
            }
            "smtp_inspect_policy" => {
                self.smtp_inspect_policy = g3_yaml::value::as_protocol_inspect_policy(v)
                    .context(format!("invalid protocol inspect policy value for key {k}"))?;
                Ok(())
            }
            "smtp_interception" => {
                self.smtp_interception = g3_yaml::value::as_smtp_interception_config(v)
                    .context(format!("invalid smtp interception value for key {k}"))?;
                Ok(())
            }
            "icap_reqmod_service" => {
                let service = g3_yaml::value::as_icap_reqmod_service_config(v).context(format!(
                    "invalid icap reqmod service config value for key {k}"
                ))?;
                self.icap_reqmod_service = Some(Arc::new(service));
                Ok(())
            }
            "icap_respmod_service" => {
                let service = g3_yaml::value::as_icap_respmod_service_config(v).context(
                    format!("invalid icap respmod service config value for key {k}"),
                )?;
                self.icap_respmod_service = Some(Arc::new(service));
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
