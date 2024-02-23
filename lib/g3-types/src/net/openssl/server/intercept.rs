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

use std::time::Duration;

use anyhow::anyhow;
use openssl::ex_data::Index;
use openssl::ssl::{AlpnError, Ssl, SslAcceptor, SslContext, SslMethod, SslRef, TlsExtType};

use super::{DEFAULT_ACCEPT_TIMEOUT, MINIMAL_ACCEPT_TIMEOUT};
use crate::net::{TlsAlpn, TlsServerName};

pub struct OpensslInterceptionServerConfig {
    sni_index: Index<Ssl, TlsServerName>,
    alpn_index: Index<Ssl, TlsAlpn>,
    alpn_name_index: Index<Ssl, Vec<u8>>,
    pub ssl_context: SslContext,
    pub accept_timeout: Duration,
}

impl OpensslInterceptionServerConfig {
    #[inline]
    pub fn server_name<'a>(&self, ssl: &'a SslRef) -> Option<&'a TlsServerName> {
        ssl.ex_data(self.sni_index)
    }

    #[inline]
    pub fn alpn_extension<'a>(&self, ssl: &'a SslRef) -> Option<&'a TlsAlpn> {
        ssl.ex_data(self.alpn_index)
    }

    pub fn set_selected_alpn(&self, ssl: &mut SslRef, protocol_name: Vec<u8>) {
        ssl.set_ex_data(self.alpn_name_index, protocol_name);
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpensslInterceptionServerConfigBuilder {
    accept_timeout: Duration,
}

impl Default for OpensslInterceptionServerConfigBuilder {
    fn default() -> Self {
        OpensslInterceptionServerConfigBuilder {
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
        let retry_index =
            Ssl::new_ex_index().map_err(|e| anyhow!("failed to create ex index: {e}"))?;
        let sni_index =
            Ssl::new_ex_index().map_err(|e| anyhow!("failed to create ex index: {e}"))?;
        let alpn_index =
            Ssl::new_ex_index().map_err(|e| anyhow!("failed to create ex index: {e}"))?;
        let alpn_name_index: Index<Ssl, Vec<u8>> =
            Ssl::new_ex_index().map_err(|e| anyhow!("failed to create ex index: {e}"))?;

        let mut builder = SslAcceptor::mozilla_intermediate_v5(SslMethod::tls_server())
            .map_err(|e| anyhow!("failed to get ssl acceptor builder: {e}"))?;

        #[cfg(not(any(feature = "boringssl", feature = "aws-lc")))]
        builder.set_client_hello_callback(move |ssl, alert| {
            use openssl::ssl::{ClientHelloError, SslAlert};

            if ssl.ex_data(retry_index).is_some() {
                return Ok(());
            }
            ssl.set_ex_data(retry_index, ());

            if let Some(sni_ext) = ssl.client_hello_ext(TlsExtType::SERVER_NAME) {
                if let Ok(name) = TlsServerName::from_extension_value(sni_ext) {
                    ssl.set_ex_data(sni_index, name);
                } else {
                    *alert = SslAlert::DECODE_ERROR;
                    return Err(ClientHelloError::ERROR);
                }
            }
            if let Some(alpn_ext) = ssl.client_hello_ext(TlsExtType::ALPN) {
                if let Ok(alpn) = TlsAlpn::from_extension_value(alpn_ext) {
                    ssl.set_ex_data(alpn_index, alpn);
                } else {
                    *alert = SslAlert::DECODE_ERROR;
                    return Err(ClientHelloError::ERROR);
                }
            }

            Err(ClientHelloError::RETRY)
        });
        #[cfg(any(feature = "boringssl", feature = "aws-lc"))]
        builder.set_select_certificate_callback(move |mut ch| {
            use openssl::ssl::SelectCertError;

            if ch.ssl().ex_data(retry_index).is_some() {
                return Ok(());
            }
            ch.ssl_mut().set_ex_data(retry_index, ());

            if let Some(sni_ext) = ch.get_extension(TlsExtType::SERVER_NAME) {
                if let Ok(name) = TlsServerName::from_extension_value(sni_ext) {
                    ch.ssl_mut().set_ex_data(sni_index, name);
                } else {
                    return Err(SelectCertError::ERROR);
                }
            }
            if let Some(alpn_ext) = ch.get_extension(TlsExtType::ALPN) {
                if let Ok(alpn) = TlsAlpn::from_extension_value(alpn_ext) {
                    ch.ssl_mut().set_ex_data(alpn_index, alpn);
                } else {
                    return Err(SelectCertError::ERROR);
                }
            }

            Err(SelectCertError::RETRY)
        });

        builder.set_alpn_select_callback(move |ssl, client_p| match ssl.ex_data(alpn_name_index) {
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
        });

        Ok(OpensslInterceptionServerConfig {
            sni_index,
            alpn_index,
            alpn_name_index,
            ssl_context: builder.build().into_context(),
            accept_timeout: self.accept_timeout,
        })
    }
}
