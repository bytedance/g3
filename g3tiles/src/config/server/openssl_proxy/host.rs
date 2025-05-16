/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use openssl::ex_data::Index;
use openssl::ssl::{
    SslAcceptor, SslAcceptorBuilder, SslContext, SslContextBuilder, SslOptions,
    SslSessionCacheMode, SslVerifyMode, TicketKeyStatus,
};
use openssl::stack::Stack;
use openssl::x509::X509;
use openssl::x509::store::X509StoreBuilder;
use std::sync::Arc;
use yaml_rust::Yaml;

use g3_types::collection::NamedValue;
use g3_types::limit::RateLimitQuotaConfig;
use g3_types::metrics::NodeName;
use g3_types::net::{
    OpensslCertificatePair, OpensslServerSessionCache, OpensslSessionIdContext, OpensslTicketKey,
    RollingTicketer, TcpSockSpeedLimitConfig,
};
use g3_types::route::AlpnMatch;
use g3_yaml::{YamlDocPosition, YamlMapCallback};

#[cfg(feature = "vendored-tongsuo")]
use g3_types::net::OpensslTlcpCertificatePair;

#[derive(Clone, Debug, Default, PartialEq)]
pub(crate) struct OpensslHostConfig {
    name: String,
    cert_pairs: Vec<OpensslCertificatePair>,
    #[cfg(feature = "vendored-tongsuo")]
    tlcp_cert_pairs: Vec<OpensslTlcpCertificatePair>,
    client_auth: bool,
    client_auth_certs: Vec<Vec<u8>>,
    session_id_context: String,
    no_session_ticket: bool,
    no_session_cache: bool,
    pub(crate) request_alive_max: Option<usize>,
    pub(crate) request_rate_limit: Option<RateLimitQuotaConfig>,
    pub(crate) tcp_sock_speed_limit: Option<TcpSockSpeedLimitConfig>,
    pub(crate) task_idle_max_count: Option<usize>,
    pub(crate) backends: AlpnMatch<NodeName>,
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

    fn set_client_auth(
        &self,
        ssl_builder: &mut SslContextBuilder,
        id_ctx: &mut OpensslSessionIdContext,
    ) -> anyhow::Result<()> {
        if self.client_auth {
            ssl_builder.set_verify(SslVerifyMode::PEER | SslVerifyMode::FAIL_IF_NO_PEER_CERT);

            let mut store_builder = X509StoreBuilder::new()
                .map_err(|e| anyhow!("failed to create ca cert store builder: {e}"))?;
            let mut subject_stack =
                Stack::new().map_err(|e| anyhow!("failed to get new ca name stack: {e}"))?;

            if self.client_auth_certs.is_empty() {
                store_builder
                    .set_default_paths()
                    .map_err(|e| anyhow!("failed to load default ca certs: {e}"))?;
            } else {
                for (i, cert) in self.client_auth_certs.iter().enumerate() {
                    let ca_cert = X509::from_der(cert.as_slice()).unwrap();
                    let subject = ca_cert
                        .subject_name()
                        .to_owned()
                        .map_err(|e| anyhow!("[#{i}] failed to get ca subject name: {e}"))?;
                    id_ctx
                        .add_ca_subject(&subject)
                        .map_err(|e| anyhow!("#[{i}]: failed to add to session id context: {e}"))?;
                    store_builder
                        .add_cert(ca_cert)
                        .map_err(|e| anyhow!("[#{i}] failed to add ca certificate: {e}"))?;
                    subject_stack
                        .push(subject)
                        .map_err(|e| anyhow!("[#{i}] failed to push to ca name stack: {e}"))?;
                }
            }
            let store = store_builder.build();
            ssl_builder
                .set_verify_cert_store(store)
                .map_err(|e| anyhow!("failed to set verify ca certs: {e}"))?;
            if !subject_stack.is_empty() {
                ssl_builder.set_client_ca_list(subject_stack);
            }
        } else {
            ssl_builder.set_verify(SslVerifyMode::NONE);
        }

        Ok(())
    }

    pub(crate) fn build_ssl_context(
        &self,
        ticketer: Option<Arc<RollingTicketer<OpensslTicketKey>>>,
    ) -> anyhow::Result<Option<SslContext>> {
        if self.cert_pairs.is_empty() {
            return Ok(None);
        }

        let mut id_ctx = OpensslSessionIdContext::new()
            .map_err(|e| anyhow!("failed to create session id context builder: {e}"))?;
        if !self.session_id_context.is_empty() {
            id_ctx
                .add_text(&self.session_id_context)
                .map_err(|e| anyhow!("failed to add session id context text: {e}"))?;
        }

        #[cfg(not(feature = "vendored-tongsuo"))]
        let mut ssl_builder =
            SslAcceptor::mozilla_intermediate_v5(openssl::ssl::SslMethod::tls_server())
                .map_err(|e| anyhow!("failed to build ssl context: {e}"))?;
        #[cfg(feature = "vendored-tongsuo")]
        let mut ssl_builder =
            SslAcceptor::tongsuo_tls().map_err(|e| anyhow!("failed to build ssl context: {e}"))?;

        if self.no_session_cache {
            ssl_builder.set_session_cache_mode(SslSessionCacheMode::OFF);
        } else {
            let cache = OpensslServerSessionCache::new(256)?;
            cache.add_to_context(&mut ssl_builder);
        }
        if self.no_session_ticket {
            ssl_builder.set_options(SslOptions::NO_TICKET);
        } else if let Some(ticketer) = ticketer {
            let ticket_key_index = SslContext::new_ex_index()
                .map_err(|e| anyhow!("failed to create ex index: {e}"))?;
            ssl_builder.set_ex_data(ticket_key_index, ticketer);
            set_ticket_key_callback(&mut ssl_builder, ticket_key_index)?;
        }

        self.set_client_auth(&mut ssl_builder, &mut id_ctx)?;

        // ssl_builder.set_mode() // TODO do we need it?
        // ssl_builder.set_options() // TODO do we need it?

        for (i, pair) in self.cert_pairs.iter().enumerate() {
            pair.add_to_server_ssl_context(&mut ssl_builder, &mut id_ctx)
                .context(format!("failed to add cert pair #{i} to ssl context"))?;
        }

        id_ctx
            .build_set(&mut ssl_builder)
            .map_err(|e| anyhow!("failed to set session id context: {e}"))?;

        if !self.backends.is_empty() {
            let mut buf = Vec::with_capacity(32);
            self.backends.protocols().iter().for_each(|p| {
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

        Ok(Some(ssl_acceptor.into_context()))
    }

    #[cfg(feature = "vendored-tongsuo")]
    pub(crate) fn build_tlcp_context(
        &self,
        ticketer: Option<Arc<RollingTicketer<OpensslTicketKey>>>,
    ) -> anyhow::Result<Option<SslContext>> {
        if self.tlcp_cert_pairs.is_empty() {
            return Ok(None);
        }

        let mut id_ctx = OpensslSessionIdContext::new()
            .map_err(|e| anyhow!("failed to create session id context builder: {e}"))?;
        if !self.session_id_context.is_empty() {
            id_ctx
                .add_text(&self.session_id_context)
                .map_err(|e| anyhow!("failed to add session id context text: {e}"))?;
        }

        let mut ssl_builder =
            SslAcceptor::tongsuo_tlcp().map_err(|e| anyhow!("failed to build ssl context: {e}"))?;

        if self.no_session_cache {
            ssl_builder.set_session_cache_mode(SslSessionCacheMode::OFF);
        } else {
            let cache = OpensslServerSessionCache::new(256)?;
            cache.add_to_context(&mut ssl_builder);
        }
        if self.no_session_ticket {
            ssl_builder.set_options(SslOptions::NO_TICKET);
        } else if let Some(ticketer) = ticketer {
            let ticket_key_index = SslContext::new_ex_index()
                .map_err(|e| anyhow!("failed to create ex index: {e}"))?;
            ssl_builder.set_ex_data(ticket_key_index, ticketer);
            set_ticket_key_callback(&mut ssl_builder, ticket_key_index)?;
        }

        self.set_client_auth(&mut ssl_builder, &mut id_ctx)?;

        for (i, pair) in self.tlcp_cert_pairs.iter().enumerate() {
            pair.add_to_server_ssl_context(&mut ssl_builder, &mut id_ctx)
                .context(format!("failed to add tlcp cert pair #{i} to ssl context"))?;
        }

        id_ctx
            .build_set(&mut ssl_builder)
            .map_err(|e| anyhow!("failed to set session id context: {e}"))?;

        if !self.backends.is_empty() {
            let mut buf = Vec::with_capacity(32);
            self.backends.protocols().iter().for_each(|p| {
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

        Ok(Some(ssl_builder.build().into_context()))
    }
}

fn set_ticket_key_callback(
    builder: &mut SslAcceptorBuilder,
    ticket_key_index: Index<SslContext, Arc<RollingTicketer<OpensslTicketKey>>>,
) -> anyhow::Result<()> {
    builder
        .set_ticket_key_callback(move |ssl, name, iv, cipher_ctx, hmac_ctx, is_enc| {
            match ssl.ssl_context().ex_data(ticket_key_index) {
                Some(ticketer) => {
                    if is_enc {
                        ticketer.encrypt_init(name, iv, cipher_ctx, hmac_ctx)
                    } else {
                        ticketer.decrypt_init(name, iv, cipher_ctx, hmac_ctx)
                    }
                }
                None => Ok(TicketKeyStatus::FAILED),
            }
        })
        .map_err(|e| anyhow!("failed to set ticket key callback: {e}"))
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
                self.cert_pairs = g3_yaml::value::as_list(value, |v| {
                    g3_yaml::value::as_openssl_certificate_pair(v, Some(lookup_dir))
                })
                .context(format!(
                    "invalid openssl cert pair list value for key {key}"
                ))?;
                Ok(())
            }
            #[cfg(feature = "vendored-tongsuo")]
            "tlcp_cert_pairs" => {
                let lookup_dir = g3_daemon::config::get_lookup_dir(doc)?;
                self.tlcp_cert_pairs = g3_yaml::value::as_list(value, |v| {
                    g3_yaml::value::as_openssl_tlcp_certificate_pair(v, Some(lookup_dir))
                })
                .context(format!(
                    "invalid openssl tlcp cert pair list value for key {key}"
                ))?;
                Ok(())
            }
            "enable_client_auth" => {
                self.client_auth = g3_yaml::value::as_bool(value)
                    .context(format!("invalid value for key {key}"))?;
                Ok(())
            }
            "session_id_context" => {
                self.session_id_context = g3_yaml::value::as_string(value)?;
                Ok(())
            }
            "no_session_ticket" | "disable_session_ticket" => {
                self.no_session_ticket = g3_yaml::value::as_bool(value)?;
                Ok(())
            }
            "no_session_cache" | "disable_session_cache" => {
                self.no_session_cache = g3_yaml::value::as_bool(value)?;
                Ok(())
            }
            "ca_certificate" | "ca_cert" | "client_auth_certificate" | "client_auth_cert" => {
                let lookup_dir = g3_daemon::config::get_lookup_dir(doc)?;
                let certs = g3_yaml::value::as_openssl_certificates(value, Some(lookup_dir))
                    .context(format!("invalid certificate(s) value for key {key}"))?;
                self.set_client_auth_certificates(certs)
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
                let max_count = g3_yaml::value::as_usize(value)
                    .context(format!("invalid usize value for key {key}"))?;
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
        #[cfg(not(feature = "vendored-tongsuo"))]
        if self.cert_pairs.is_empty() {
            return Err(anyhow!("no certificate set"));
        }
        #[cfg(feature = "vendored-tongsuo")]
        if self.cert_pairs.is_empty() && self.tlcp_cert_pairs.is_empty() {
            return Err(anyhow!("neither tls nor tlcp certificate set"));
        }
        if self.backends.is_empty() {
            return Err(anyhow!("no backend service set"));
        }
        Ok(())
    }
}
