/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use openssl::ex_data::Index;
use openssl::ssl::{AlpnError, Ssl, SslAcceptor, SslAcceptorBuilder, SslContext, SslRef};

use super::{DEFAULT_ACCEPT_TIMEOUT, MINIMAL_ACCEPT_TIMEOUT, OpensslTicketKey};
use crate::net::RollingTicketer;

pub struct OpensslInterceptionServerConfig {
    alpn_name_index: Index<Ssl, Vec<u8>>,
    pub ssl_context: SslContext,
    #[cfg(tongsuo)]
    pub tlcp_context: SslContext,
    pub client_hello_recv_timeout: Duration,
    pub client_hello_max_size: u32,
    pub accept_timeout: Duration,
}

impl OpensslInterceptionServerConfig {
    pub fn set_selected_alpn(&self, ssl: &mut SslRef, protocol_name: Vec<u8>) {
        ssl.set_ex_data(self.alpn_name_index, protocol_name);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpensslInterceptionServerConfigBuilder {
    client_hello_recv_timeout: Duration,
    client_hello_max_size: u32,
    accept_timeout: Duration,
}

impl Default for OpensslInterceptionServerConfigBuilder {
    fn default() -> Self {
        OpensslInterceptionServerConfigBuilder {
            client_hello_recv_timeout: Duration::from_secs(10),
            client_hello_max_size: 16384,
            accept_timeout: DEFAULT_ACCEPT_TIMEOUT,
        }
    }
}

impl OpensslInterceptionServerConfigBuilder {
    pub fn check(&mut self) -> anyhow::Result<()> {
        if self.accept_timeout < MINIMAL_ACCEPT_TIMEOUT {
            self.accept_timeout = MINIMAL_ACCEPT_TIMEOUT;
        }

        Ok(())
    }

    pub fn set_accept_timeout(&mut self, timeout: Duration) {
        self.accept_timeout = timeout;
    }

    pub fn build(&self) -> anyhow::Result<OpensslInterceptionServerConfig> {
        self.build_with_ticketer(None)
    }

    pub fn build_with_ticketer(
        &self,
        ticketer: Option<&Arc<RollingTicketer<OpensslTicketKey>>>,
    ) -> anyhow::Result<OpensslInterceptionServerConfig> {
        let alpn_name_index: Index<Ssl, Vec<u8>> =
            Ssl::new_ex_index().map_err(|e| anyhow!("failed to create ex index: {e}"))?;
        let ticket_key_index: Index<SslContext, Arc<RollingTicketer<OpensslTicketKey>>> =
            SslContext::new_ex_index().map_err(|e| anyhow!("failed to create ex index: {e}"))?;

        macro_rules! build_ssl_context {
            ($method:expr) => {{
                let mut builder = $method()?;
                set_alpn_select_callback(&mut builder, alpn_name_index);
                if let Some(ticketer) = ticketer {
                    builder.set_ex_data(ticket_key_index, ticketer.clone());
                    super::set_ticket_key_callback(&mut builder, ticket_key_index)?;
                }
                builder.build().into_context()
            }};
        }

        let ssl_context = build_ssl_context!(build_tls_context);
        #[cfg(tongsuo)]
        let tlcp_context = build_ssl_context!(build_tlcp_context);

        Ok(OpensslInterceptionServerConfig {
            alpn_name_index,
            ssl_context,
            #[cfg(tongsuo)]
            tlcp_context,
            client_hello_recv_timeout: self.client_hello_recv_timeout,
            client_hello_max_size: self.client_hello_max_size,
            accept_timeout: self.accept_timeout,
        })
    }
}

#[cfg(not(tongsuo))]
fn build_tls_context() -> anyhow::Result<SslAcceptorBuilder> {
    use openssl::ssl::SslMethod;

    SslAcceptor::mozilla_intermediate_v5(SslMethod::tls_server())
        .map_err(|e| anyhow!("failed to get ssl acceptor builder: {e}"))
}

#[cfg(tongsuo)]
fn build_tls_context() -> anyhow::Result<SslAcceptorBuilder> {
    SslAcceptor::tongsuo_tls().map_err(|e| anyhow!("failed to get tls acceptor builder: {e}"))
}

#[cfg(tongsuo)]
fn build_tlcp_context() -> anyhow::Result<SslAcceptorBuilder> {
    SslAcceptor::tongsuo_tlcp().map_err(|e| anyhow!("failed to get tlcp acceptor builder: {e}"))
}

fn set_alpn_select_callback(
    builder: &mut SslAcceptorBuilder,
    alpn_name_index: Index<Ssl, Vec<u8>>,
) {
    builder.set_alpn_select_callback(move |ssl: &mut SslRef, client_p: &[u8]| {
        match ssl.ex_data(alpn_name_index) {
            Some(protocol_name) => {
                let mut offset = 0;
                while offset < client_p.len() {
                    let name_len = client_p[offset] as usize;
                    let end = offset + 1 + name_len;
                    if end > client_p.len() {
                        return Err(AlpnError::ALERT_FATAL);
                    }
                    let name = &client_p[offset + 1..end];
                    if name == protocol_name.as_slice() {
                        return Ok(name);
                    }
                    offset = end;
                }

                Err(AlpnError::ALERT_FATAL)
            }
            None => Err(AlpnError::NOACK),
        }
    });
}
