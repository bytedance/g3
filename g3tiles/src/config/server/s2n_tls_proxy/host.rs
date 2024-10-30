/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use std::time::SystemTime;

use anyhow::{anyhow, Context};
use s2n_tls::enums::ClientAuthType;
use yaml_rust::Yaml;

use g3_types::collection::NamedValue;
use g3_types::limit::RateLimitQuotaConfig;
use g3_types::metrics::MetricsName;
use g3_types::net::{TcpSockSpeedLimitConfig, UnparsedTlsCertPair};
use g3_types::route::AlpnMatch;
use g3_yaml::{YamlDocPosition, YamlMapCallback};

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct S2nTlsHostConfig {
    name: String,
    cert_pairs: Vec<UnparsedTlsCertPair>,
    client_auth: bool,
    client_auth_certs: Vec<String>,
    use_session_ticket: bool,
    pub(crate) request_alive_max: Option<usize>,
    pub(crate) request_rate_limit: Option<RateLimitQuotaConfig>,
    pub(crate) tcp_sock_speed_limit: Option<TcpSockSpeedLimitConfig>,
    pub(crate) task_idle_max_count: Option<i32>,
    pub(crate) backends: AlpnMatch<MetricsName>,
}

impl Default for S2nTlsHostConfig {
    fn default() -> Self {
        S2nTlsHostConfig {
            name: String::new(),
            cert_pairs: Vec::with_capacity(1),
            client_auth: false,
            client_auth_certs: Vec::new(),
            use_session_ticket: false,
            request_alive_max: None,
            request_rate_limit: None,
            tcp_sock_speed_limit: None,
            task_idle_max_count: None,
            backends: AlpnMatch::default(),
        }
    }
}

impl NamedValue for S2nTlsHostConfig {
    type Name = str;
    type NameOwned = String;

    fn name(&self) -> &Self::Name {
        self.name.as_str()
    }

    fn name_owned(&self) -> Self::NameOwned {
        self.name.clone()
    }
}

impl S2nTlsHostConfig {
    pub(crate) fn build_tls_config(&self) -> anyhow::Result<s2n_tls::config::Config> {
        let mut builder = s2n_tls::config::Builder::new();
        builder
            .set_security_policy(&s2n_tls::security::DEFAULT_TLS13)
            .map_err(|e| anyhow!("failed to set security policy: {e}"))?;

        if self.client_auth {
            builder
                .set_client_auth_type(ClientAuthType::Required)
                .map_err(|e| anyhow!("failed to enable client auth: {e}"))?;
            if !self.client_auth_certs.is_empty() {
                builder
                    .wipe_trust_store()
                    .map_err(|e| anyhow!("failed to wipe default trusted CA certs: {e}"))?;
            }
            for cert in &self.client_auth_certs {
                builder
                    .trust_pem(cert.as_bytes())
                    .map_err(|e| anyhow!("failed to add client auth CA cert: {e}"))?;
            }
        } else {
            builder.with_system_certs(false).map_err(|e| {
                anyhow!("failed to disable the load of system default ca certs: {e}")
            })?;
        }

        for (i, pair) in self.cert_pairs.iter().enumerate() {
            builder
                .load_pem(pair.cert_chain(), pair.private_key())
                .map_err(|e| anyhow!("failed to load cert and key pair {i}: {e}"))?;
        }

        // TODO set session storage
        if self.use_session_ticket {
            // TODO rotate session ticket key
            builder
                .add_session_ticket_key(b"test", b"1234567890abcdef", SystemTime::now())
                .map_err(|e| anyhow!("failed to add session ticket key: {e}"))?;
        }

        if !self.backends.is_empty() {
            builder
                .set_application_protocol_preference(self.backends.protocols().clone())
                .map_err(|e| anyhow!("failed to set ALPN list: {e}"))?;
        }

        builder
            .build()
            .map_err(|e| anyhow!("failed to build tls config: {e}"))
    }
}

impl YamlMapCallback for S2nTlsHostConfig {
    fn type_name(&self) -> &'static str {
        "S2nTlsHostConfig"
    }

    fn parse_kv(
        &mut self,
        key: &str,
        value: &Yaml,
        doc: Option<&YamlDocPosition>,
    ) -> anyhow::Result<()> {
        match g3_yaml::key::normalize(key).as_str() {
            "name" => {
                self.name = g3_yaml::value::as_string(value)?;
                Ok(())
            }
            "cert_pairs" => {
                let lookup_dir = g3_daemon::config::get_lookup_dir(doc)?;
                self.cert_pairs = g3_yaml::value::as_list(value, |v| {
                    g3_yaml::value::as_s2n_tls_certificate_pair(v, Some(lookup_dir))
                })
                .context(format!(
                    "invalid s2n tls cert pair list value for key {key}"
                ))?;
                Ok(())
            }
            "enable_client_auth" => {
                self.client_auth = g3_yaml::value::as_bool(value)
                    .context(format!("invalid value for key {key}"))?;
                Ok(())
            }
            "use_session_ticket" => {
                self.use_session_ticket = g3_yaml::value::as_bool(value)
                    .context(format!("invalid value for key {key}"))?;
                Ok(())
            }
            "ca_certificate" | "ca_cert" | "client_auth_certificate" | "client_auth_cert" => {
                let lookup_dir = g3_daemon::config::get_lookup_dir(doc)?;
                let certs = g3_yaml::value::as_s2n_tls_certificates(value, Some(lookup_dir))
                    .context(format!("invalid certificate(s) value for key {key}"))?;
                self.client_auth_certs.extend(certs);
                Ok(())
            }
            "request_rate_limit" | "request_limit_quota" => {
                let quota = g3_yaml::value::as_rate_limit_quota(value)
                    .context(format!("invalid request quota value for key {key}"))?;
                self.request_rate_limit = Some(quota);
                Ok(())
            }
            "request_max_alive" | "request_alive_max" => {
                let alive_max = g3_yaml::value::as_usize(value)
                    .context(format!("invalid usize value for key {key}"))?;
                self.request_alive_max = Some(alive_max);
                Ok(())
            }
            "tcp_sock_speed_limit" | "tcp_conn_speed_limit" => {
                let limit = g3_yaml::value::as_tcp_sock_speed_limit(value).context(format!(
                    "invalid tcp socket speed limit value for key {key}"
                ))?;
                self.tcp_sock_speed_limit = Some(limit);
                Ok(())
            }
            "task_idle_max_count" => {
                let max_count = g3_yaml::value::as_i32(value)
                    .context(format!("invalid i32 value for key {key}"))?;
                self.task_idle_max_count = Some(max_count);
                Ok(())
            }
            "backends" => {
                self.backends = g3_yaml::value::as_alpn_matched_backends(value)?;
                Ok(())
            }
            _ => Err(anyhow!("invalid key {key}")),
        }
    }

    fn check(&mut self) -> anyhow::Result<()> {
        if self.name.is_empty() {
            return Err(anyhow!("no name set"));
        }
        if self.cert_pairs.is_empty() {
            return Err(anyhow!("no certificate set"));
        }
        if self.backends.is_empty() {
            return Err(anyhow!("no backend service set"));
        }
        Ok(())
    }
}
