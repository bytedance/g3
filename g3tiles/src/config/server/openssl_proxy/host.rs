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
use openssl::ssl::{SslAcceptor, SslContext, SslMethod, SslSessionCacheMode, SslVerifyMode};
use openssl::stack::Stack;
use openssl::x509::store::X509StoreBuilder;
use openssl::x509::X509;
use yaml_rust::Yaml;

use g3_types::collection::NamedValue;
use g3_types::limit::RateLimitQuotaConfig;
use g3_types::net::{OpensslCertificatePair, TcpSockSpeedLimitConfig};
use g3_types::route::AlpnMatch;
use g3_yaml::{YamlDocPosition, YamlMapCallback};

use super::OpensslServiceConfig;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct OpensslHostConfig {
    name: String,
    cert_pairs: Vec<OpensslCertificatePair>,
    client_auth: bool,
    client_auth_certs: Vec<Vec<u8>>,
    pub(crate) request_alive_max: Option<usize>,
    pub(crate) request_rate_limit: Option<RateLimitQuotaConfig>,
    pub(crate) tcp_sock_speed_limit: Option<TcpSockSpeedLimitConfig>,
    pub(crate) task_idle_max_count: Option<i32>,
    pub(crate) services: AlpnMatch<Arc<OpensslServiceConfig>>,
}

impl NamedValue for OpensslHostConfig {
    type Name = str;
    type NameOwned = String;

    fn name(&self) -> &Self::Name {
        self.name.as_str()
    }

    fn name_owned(&self) -> Self::NameOwned {
        self.name.clone()
    }
}

impl OpensslHostConfig {
    fn set_client_auth_certificates(&mut self, certs: Vec<X509>) -> anyhow::Result<()> {
        for (i, cert) in certs.into_iter().enumerate() {
            let bytes = cert
                .to_der()
                .map_err(|e| anyhow!("failed to encode client chain certificate #{i}: {e}"))?;
            self.client_auth_certs.push(bytes);
        }
        Ok(())
    }

    pub(crate) fn build_ssl_context(&self) -> anyhow::Result<SslContext> {
        let mut ssl_builder = SslAcceptor::mozilla_intermediate_v5(SslMethod::tls_server())
            .map_err(|e| anyhow!("failed to build ssl context: {e}"))?;

        ssl_builder.set_session_cache_mode(SslSessionCacheMode::SERVER); // TODO use external cache?

        if self.client_auth {
            ssl_builder.set_verify(SslVerifyMode::PEER | SslVerifyMode::FAIL_IF_NO_PEER_CERT);

            let mut store_builder = X509StoreBuilder::new()
                .map_err(|e| anyhow!("failed to create ca cert store builder: {e}"))?;
            if self.client_auth_certs.is_empty() {
                store_builder
                    .set_default_paths()
                    .map_err(|e| anyhow!("failed to load default ca certs: {e}"))?;
            } else {
                for (i, cert) in self.client_auth_certs.iter().enumerate() {
                    let ca_cert = X509::from_der(cert.as_slice()).unwrap();
                    store_builder
                        .add_cert(ca_cert)
                        .map_err(|e| anyhow!("[#{i}] failed to add ca certificate: {e}"))?;
                }
            }
            let store = store_builder.build();

            let mut ca_stack =
                Stack::new().map_err(|e| anyhow!("failed to get new ca name stack: {e}"))?;
            for (i, obj) in store.objects().iter().enumerate() {
                if let Some(cert) = obj.x509() {
                    let name = cert
                        .subject_name()
                        .to_owned()
                        .map_err(|e| anyhow!("[#{i}] failed to get subject name: {e}"))?;
                    ca_stack
                        .push(name)
                        .map_err(|e| anyhow!("[#{i}] failed to push to ca name stack: {e}"))?;
                }
            }

            ssl_builder.set_client_ca_list(ca_stack);
            ssl_builder
                .set_verify_cert_store(store)
                .map_err(|e| anyhow!("failed to set ca certs: {e}"))?;
        } else {
            ssl_builder.set_verify(SslVerifyMode::NONE);
        }

        // ssl_builder.set_mode() // TODO do we need it?
        // ssl_builder.set_options() // TODO do we need it?

        for (i, pair) in self.cert_pairs.iter().enumerate() {
            pair.add_to_ssl_context(&mut ssl_builder)
                .context(format!("failed to add cert pair #{i} to ssl context"))?;
        }

        if !self.services.is_empty() {
            let mut buf = Vec::with_capacity(32);
            self.services.protocols().iter().for_each(|p| {
                if let Ok(len) = u8::try_from(p.len()) {
                    buf.push(len);
                    buf.extend_from_slice(p.as_bytes());
                }
            });
            if !buf.is_empty() {
                ssl_builder
                    .set_alpn_protos(buf.as_slice())
                    .map_err(|e| anyhow!("failed to set alpn protocols: {e}"))?;
            }
        }

        let ssl_acceptor = ssl_builder.build();

        Ok(ssl_acceptor.into_context())
    }
}

impl YamlMapCallback for OpensslHostConfig {
    fn type_name(&self) -> &'static str {
        "OpensslHostConfig"
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
                        let pair = g3_yaml::value::as_openssl_certificate_pair(v, Some(lookup_dir))
                            .context(format!("invalid openssl cert pair value for {key}#{i}"))?;
                        self.cert_pairs.push(pair);
                    }
                } else {
                    let pair = g3_yaml::value::as_openssl_certificate_pair(value, Some(lookup_dir))
                        .context(format!("invalid openssl cert pair value for key {key}"))?;
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
                let certs = g3_yaml::value::as_openssl_certificates(value, Some(lookup_dir))
                    .context(format!("invalid certificate(s) value for key {key}"))?;
                self.set_client_auth_certificates(certs)?;
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
