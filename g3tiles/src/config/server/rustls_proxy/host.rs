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
use std::time::Duration;

use anyhow::{anyhow, Context};
use rustls::server::AllowAnyAuthenticatedClient;
use rustls::{Certificate, RootCertStore, ServerConfig};
use yaml_rust::Yaml;

use g3_types::collection::NamedValue;
use g3_types::limit::RateLimitQuotaConfig;
use g3_types::net::{MultipleCertResolver, RustlsCertificatePair, TcpSockSpeedLimitConfig};
use g3_types::route::AlpnMatch;
use g3_yaml::{YamlDocPosition, YamlMapCallback};

use super::RustlsServiceConfig;

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct RustlsHostConfig {
    name: String,
    cert_pairs: Vec<RustlsCertificatePair>,
    client_auth: bool,
    client_auth_certs: Vec<Certificate>,
    pub(crate) accept_timeout: Duration,
    pub(crate) request_alive_max: Option<usize>,
    pub(crate) request_rate_limit: Option<RateLimitQuotaConfig>,
    pub(crate) tcp_sock_speed_limit: Option<TcpSockSpeedLimitConfig>,
    pub(crate) task_idle_max_count: Option<i32>,
    pub(crate) services: AlpnMatch<Arc<RustlsServiceConfig>>,
}

impl Default for RustlsHostConfig {
    fn default() -> Self {
        RustlsHostConfig {
            name: String::new(),
            cert_pairs: Vec::with_capacity(1),
            client_auth: false,
            client_auth_certs: Vec::new(),
            accept_timeout: Duration::from_secs(60),
            request_alive_max: None,
            request_rate_limit: None,
            tcp_sock_speed_limit: None,
            task_idle_max_count: None,
            services: AlpnMatch::default(),
        }
    }
}

impl NamedValue for RustlsHostConfig {
    type Name = str;
    type NameOwned = String;

    fn name(&self) -> &Self::Name {
        self.name.as_str()
    }

    fn name_owned(&self) -> Self::NameOwned {
        self.name.clone()
    }
}

impl RustlsHostConfig {
    pub(crate) fn build_tls_config(&self) -> anyhow::Result<Arc<ServerConfig>> {
        let config_builder = ServerConfig::builder().with_safe_defaults();
        let config_builder = if self.client_auth {
            let mut root_store = RootCertStore::empty();
            if self.client_auth_certs.is_empty() {
                let certs = g3_types::net::load_openssl_certs_for_rustls()
                    .context("failed to load default openssl ca certs")?;
                for (i, cert) in certs.iter().enumerate() {
                    root_store.add(cert).map_err(|e| {
                        anyhow!("failed to add openssl ca cert {i} as root certs for client auth: {e:?}",)
                    })?;
                }
            } else {
                for (i, cert) in self.client_auth_certs.iter().enumerate() {
                    root_store.add(cert).map_err(|e| {
                        anyhow!("failed to add cert {i} as root certs for client auth: {e:?}",)
                    })?;
                }
            }
            config_builder.with_client_cert_verifier(AllowAnyAuthenticatedClient::new(root_store))
        } else {
            config_builder.with_no_client_auth()
        };

        let mut cert_resolver = MultipleCertResolver::with_capacity(self.cert_pairs.len());
        for (i, pair) in self.cert_pairs.iter().enumerate() {
            cert_resolver
                .push_cert_pair(pair)
                .context(format!("failed to add cert pair {i}"))?;
        }
        let mut config = config_builder.with_cert_resolver(Arc::new(cert_resolver));

        if !self.services.is_empty() {
            for protocol in self.services.protocols() {
                config.alpn_protocols.push(protocol.clone().into_bytes());
            }
        }

        Ok(Arc::new(config))
    }
}

impl YamlMapCallback for RustlsHostConfig {
    fn type_name(&self) -> &'static str {
        "RustlsHostConfig"
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
                if let Yaml::Array(seq) = value {
                    for (i, v) in seq.iter().enumerate() {
                        let pair = g3_yaml::value::as_rustls_certificate_pair(v, Some(lookup_dir))
                            .context(format!("invalid rustls cert pair value for {key}#{i}"))?;
                        self.cert_pairs.push(pair);
                    }
                } else {
                    let pair = g3_yaml::value::as_rustls_certificate_pair(value, Some(lookup_dir))
                        .context(format!("invalid rustls cert pair value for key {key}"))?;
                    self.cert_pairs.push(pair);
                }
                Ok(())
            }
            "enable_client_auth" => {
                self.client_auth = g3_yaml::value::as_bool(value)
                    .context(format!("invalid value for key {key}"))?;
                Ok(())
            }
            "ca_certificate" | "ca_cert" | "client_auth_certificate" | "client_auth_cert" => {
                let lookup_dir = g3_daemon::config::get_lookup_dir(doc)?;
                let certs = g3_yaml::value::as_rustls_certificates(value, Some(lookup_dir))
                    .context(format!("invalid certificate(s) value for key {key}"))?;
                for cert in certs {
                    self.client_auth_certs.push(cert);
                }
                Ok(())
            }
            "accept_timeout" | "handshake_timeout" | "negotiation_timeout" => {
                self.accept_timeout = g3_yaml::humanize::as_duration(value)
                    .context(format!("invalid humanize duration value for key {key}"))?;
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
            "services" => {
                self.services = g3_yaml::value::as_alpn_matched_obj(value, doc)?;
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
        if self.services.is_empty() {
            return Err(anyhow!("no backend service set"));
        }
        Ok(())
    }
}
