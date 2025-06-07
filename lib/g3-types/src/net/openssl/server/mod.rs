/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, anyhow};
use bytes::BufMut;
use openssl::ex_data::Index;
use openssl::ssl::{
    SslAcceptor, SslAcceptorBuilder, SslContext, SslOptions, SslSessionCacheMode, SslVerifyMode,
    TicketKeyStatus,
};
use openssl::stack::Stack;
use openssl::x509::X509;
use openssl::x509::store::X509StoreBuilder;

use super::{OpensslCertificatePair, OpensslTlcpCertificatePair};
use crate::net::{AlpnProtocol, RollingTicketer};

mod intercept;
pub use intercept::{OpensslInterceptionServerConfig, OpensslInterceptionServerConfigBuilder};

mod ticket_key;
pub use ticket_key::{OpensslTicketKey, OpensslTicketKeyBuilder};

mod ticketer;

mod session;
pub use session::{OpensslServerSessionCache, OpensslSessionIdContext};

const MINIMAL_ACCEPT_TIMEOUT: Duration = Duration::from_millis(100);
const DEFAULT_ACCEPT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
pub struct OpensslServerConfig {
    pub ssl_context: SslContext,
    pub accept_timeout: Duration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OpensslServerConfigBuilder {
    cert_pairs: Vec<OpensslCertificatePair>,
    tlcp_cert_pairs: Vec<OpensslTlcpCertificatePair>,
    client_auth: bool,
    client_auth_certs: Vec<Vec<u8>>,
    session_id_context: String,
    no_session_ticket: bool,
    no_session_cache: bool,
    accept_timeout: Duration,
}

impl OpensslServerConfigBuilder {
    pub fn empty() -> Self {
        OpensslServerConfigBuilder {
            cert_pairs: Vec::with_capacity(1),
            tlcp_cert_pairs: Vec::with_capacity(1),
            client_auth: false,
            client_auth_certs: Vec::new(),
            session_id_context: String::new(),
            no_session_ticket: false,
            no_session_cache: false,
            accept_timeout: DEFAULT_ACCEPT_TIMEOUT,
        }
    }

    #[cfg(not(tongsuo))]
    pub fn check(&self) -> anyhow::Result<()> {
        if self.cert_pairs.is_empty() {
            return Err(anyhow!("no cert pair is set"));
        }

        Ok(())
    }

    #[cfg(tongsuo)]
    pub fn check(&self) -> anyhow::Result<()> {
        if self.cert_pairs.is_empty() && self.tlcp_cert_pairs.is_empty() {
            return Err(anyhow!("no cert pair is set"));
        }

        Ok(())
    }

    pub fn enable_client_auth(&mut self) {
        self.client_auth = true;
    }

    pub fn set_client_auth_certificates(&mut self, certs: Vec<X509>) -> anyhow::Result<()> {
        for (i, cert) in certs.into_iter().enumerate() {
            let bytes = cert
                .to_der()
                .map_err(|e| anyhow!("failed to encode client chain certificate #{i}: {e}"))?;
            self.client_auth_certs.push(bytes);
        }
        Ok(())
    }

    pub fn set_session_id_context(&mut self, context: String) {
        self.session_id_context = context;
    }

    pub fn set_disable_session_ticket(&mut self, disable: bool) {
        self.no_session_ticket = disable;
    }

    pub fn set_disable_session_cache(&mut self, disable: bool) {
        self.no_session_cache = disable;
    }

    pub fn push_cert_pair(&mut self, cert_pair: OpensslCertificatePair) -> anyhow::Result<()> {
        cert_pair.check()?;
        self.cert_pairs.push(cert_pair);
        Ok(())
    }

    pub fn push_tlcp_cert_pair(
        &mut self,
        cert_pair: OpensslTlcpCertificatePair,
    ) -> anyhow::Result<()> {
        cert_pair.check()?;
        self.tlcp_cert_pairs.push(cert_pair);
        Ok(())
    }

    pub fn set_accept_timeout(&mut self, timeout: Duration) {
        self.accept_timeout = timeout;
    }

    #[cfg(not(tongsuo))]
    fn build_tls_acceptor(
        &self,
        id_ctx: &mut OpensslSessionIdContext,
    ) -> anyhow::Result<SslAcceptorBuilder> {
        use openssl::ssl::SslMethod;

        let mut ssl_builder = SslAcceptor::mozilla_intermediate_v5(SslMethod::tls_server())
            .map_err(|e| anyhow!("failed to build ssl context: {e}"))?;

        for (i, pair) in self.cert_pairs.iter().enumerate() {
            pair.add_to_server_ssl_context(&mut ssl_builder, id_ctx)
                .context(format!("failed to add cert pair #{i} to ssl context"))?;
        }

        Ok(ssl_builder)
    }

    #[cfg(tongsuo)]
    fn build_tls_acceptor(
        &self,
        id_ctx: &mut OpensslSessionIdContext,
    ) -> anyhow::Result<SslAcceptorBuilder> {
        let mut ssl_builder =
            SslAcceptor::tongsuo_tls().map_err(|e| anyhow!("failed to build ssl context: {e}"))?;

        for (i, pair) in self.cert_pairs.iter().enumerate() {
            pair.add_to_server_ssl_context(&mut ssl_builder, id_ctx)
                .context(format!("failed to add cert pair #{i} to ssl context"))?;
        }

        Ok(ssl_builder)
    }

    #[cfg(tongsuo)]
    fn build_tlcp_acceptor(
        &self,
        id_ctx: &mut OpensslSessionIdContext,
    ) -> anyhow::Result<SslAcceptorBuilder> {
        let mut ssl_builder =
            SslAcceptor::tongsuo_tlcp().map_err(|e| anyhow!("failed to build ssl context: {e}"))?;

        for (i, pair) in self.tlcp_cert_pairs.iter().enumerate() {
            pair.add_to_server_ssl_context(&mut ssl_builder, id_ctx)
                .context(format!("failed to add cert pair #{i} to ssl context"))?;
        }

        Ok(ssl_builder)
    }

    #[cfg(tongsuo)]
    fn build_acceptor(
        &self,
        id_ctx: &mut OpensslSessionIdContext,
    ) -> anyhow::Result<SslAcceptorBuilder> {
        if self.tlcp_cert_pairs.is_empty() {
            return self.build_tls_acceptor(id_ctx);
        }

        if self.cert_pairs.is_empty() {
            return self.build_tlcp_acceptor(id_ctx);
        }

        let mut ssl_builder =
            SslAcceptor::tongsuo_auto().map_err(|e| anyhow!("failed to build ssl context: {e}"))?;

        for (i, pair) in self.tlcp_cert_pairs.iter().enumerate() {
            pair.add_to_server_ssl_context(&mut ssl_builder, id_ctx)
                .context(format!("failed to add cert pair #{i} to ssl context"))?;
        }
        for (i, pair) in self.cert_pairs.iter().enumerate() {
            pair.add_to_server_ssl_context(&mut ssl_builder, id_ctx)
                .context(format!("failed to add cert pair #{i} to ssl context"))?;
        }

        Ok(ssl_builder)
    }

    #[cfg(not(tongsuo))]
    fn build_acceptor(
        &self,
        id_ctx: &mut OpensslSessionIdContext,
    ) -> anyhow::Result<SslAcceptorBuilder> {
        self.build_tls_acceptor(id_ctx)
    }

    pub fn build_with_alpn_protocols(
        &self,
        alpn_protocols: Option<Vec<AlpnProtocol>>,
        ticketer: Option<Arc<RollingTicketer<OpensslTicketKey>>>,
    ) -> anyhow::Result<OpensslServerConfig> {
        let mut id_ctx = OpensslSessionIdContext::new()
            .map_err(|e| anyhow!("failed to create session id context builder: {e}"))?;
        if !self.session_id_context.is_empty() {
            id_ctx
                .add_text(&self.session_id_context)
                .map_err(|e| anyhow!("failed to add session id context text: {e}"))?;
        }

        let mut ssl_builder = self.build_acceptor(&mut id_ctx)?;

        if self.no_session_cache {
            ssl_builder.set_session_cache_mode(SslSessionCacheMode::OFF);
        } else {
            ssl_builder.set_session_cache_mode(SslSessionCacheMode::SERVER);
        }
        if self.no_session_ticket {
            ssl_builder.set_options(SslOptions::NO_TICKET);
        } else if let Some(ticketer) = ticketer {
            let ticket_key_index = SslContext::new_ex_index()
                .map_err(|e| anyhow!("failed to create ex index: {e}"))?;
            ssl_builder.set_ex_data(ticket_key_index, ticketer);
            set_ticket_key_callback(&mut ssl_builder, ticket_key_index)?;
        }

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
                        .map_err(|e| anyhow!("[#{i}] failed to add to session id context: {e}"))?;
                    store_builder
                        .add_cert(ca_cert)
                        .map_err(|e| anyhow!("[#{i}] failed to add ca certificate: {e}"))?;
                    subject_stack
                        .push(subject)
                        .map_err(|e| anyhow!("[#{i}] failed to push to ca name stack: {e}"))?;
                }
            }
            #[cfg(not(libressl))]
            ssl_builder
                .set_verify_cert_store(store_builder.build())
                .map_err(|e| anyhow!("failed to set verify ca certs: {e}"))?;
            #[cfg(libressl)]
            ssl_builder.set_cert_store(store_builder.build());
            if !subject_stack.is_empty() {
                ssl_builder.set_client_ca_list(subject_stack);
            }
        } else {
            ssl_builder.set_verify(SslVerifyMode::NONE);
        }

        id_ctx
            .build_set(&mut ssl_builder)
            .map_err(|e| anyhow!("failed to set session id context: {e}"))?;

        // ssl_builder.set_mode() // TODO do we need it?
        // ssl_builder.set_options() // TODO do we need it?

        if let Some(protocols) = alpn_protocols {
            let mut buf = Vec::with_capacity(32);
            protocols.iter().for_each(|p| {
                buf.put_slice(p.wired_identification_sequence());
            });
            if !buf.is_empty() {
                ssl_builder
                    .set_alpn_protos(buf.as_slice())
                    .map_err(|e| anyhow!("failed to set alpn protocols: {e}"))?;
            }
        }

        let ssl_acceptor = ssl_builder.build();

        Ok(OpensslServerConfig {
            ssl_context: ssl_acceptor.into_context(),
            accept_timeout: self.accept_timeout,
        })
    }

    #[inline]
    pub fn build_with_ticketer(
        &self,
        ticketer: Option<Arc<RollingTicketer<OpensslTicketKey>>>,
    ) -> anyhow::Result<OpensslServerConfig> {
        self.build_with_alpn_protocols(None, ticketer)
    }

    #[inline]
    pub fn build(&self) -> anyhow::Result<OpensslServerConfig> {
        self.build_with_alpn_protocols(None, None)
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
