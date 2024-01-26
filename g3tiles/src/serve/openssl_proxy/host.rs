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

use anyhow::anyhow;
use arc_swap::ArcSwap;
use governor::{clock::DefaultClock, state::InMemoryState, state::NotKeyed, RateLimiter};
use openssl::ex_data::Index;
#[cfg(feature = "vendored-tongsuo")]
use openssl::ssl::SslVersion;
use openssl::ssl::{Ssl, SslAcceptor, SslContext, TlsExtType};

use g3_types::collection::NamedValue;
use g3_types::limit::{GaugeSemaphore, GaugeSemaphorePermit};
use g3_types::metrics::MetricsName;
use g3_types::net::{Host, TlsServerName};
use g3_types::route::AlpnMatch;

use crate::backend::ArcBackend;
use crate::config::server::openssl_proxy::OpensslHostConfig;

#[cfg(feature = "vendored-tongsuo")]
pub(super) fn build_lazy_ssl_context(
    version_index: Index<Ssl, SslVersion>,
    host_name_index: Index<Ssl, Host>,
) -> anyhow::Result<SslContext> {
    use openssl::ssl::ClientHelloResponse;

    let mut builder = SslAcceptor::tongsuo_auto()
        .map_err(|e| anyhow!("failed to get ssl acceptor builder: {e}"))?;

    builder.set_client_hello_callback(move |ssl, _alert| {
        let client_hello_version = ssl.client_hello_legacy_version().unwrap();
        ssl.set_ex_data(version_index, client_hello_version);

        if let Some(sni_ext) = ssl.client_hello_ext(TlsExtType::SERVER_NAME) {
            if let Ok(name) = TlsServerName::from_extension_value(sni_ext) {
                ssl.set_ex_data(host_name_index, name.into());
            }
        }

        Ok(ClientHelloResponse::RETRY)
    });
    Ok(builder.build().into_context())
}

#[cfg(not(any(feature = "vendored-tongsuo", feature = "vendored-aws-lc")))]
pub(super) fn build_lazy_ssl_context(
    host_name_index: Index<Ssl, Host>,
) -> anyhow::Result<SslContext> {
    use openssl::ssl::{ClientHelloResponse, SslMethod};

    let mut builder = SslAcceptor::mozilla_intermediate_v5(SslMethod::tls_server())
        .map_err(|e| anyhow!("failed to get ssl acceptor builder: {e}"))?;

    builder.set_client_hello_callback(move |ssl, _alert| {
        if let Some(sni_ext) = ssl.client_hello_ext(TlsExtType::SERVER_NAME) {
            if let Ok(name) = TlsServerName::from_extension_value(sni_ext) {
                ssl.set_ex_data(host_name_index, name.into());
            }
        }

        Ok(ClientHelloResponse::RETRY)
    });
    Ok(builder.build().into_context())
}

#[cfg(feature = "vendored-aws-lc")]
pub(super) fn build_lazy_ssl_context(
    host_name_index: Index<Ssl, Host>,
) -> anyhow::Result<SslContext> {
    use openssl::ssl::{SelectCertError, SslMethod};

    let mut builder = SslAcceptor::mozilla_intermediate_v5(SslMethod::tls_server())
        .map_err(|e| anyhow!("failed to get ssl acceptor builder: {e}"))?;

    builder.set_select_certificate_callback(move |mut ch| {
        if let Some(sni_ext) = ch.get_extension(TlsExtType::SERVER_NAME) {
            if let Ok(name) = TlsServerName::from_extension_value(sni_ext) {
                ch.ssl_mut().set_ex_data(host_name_index, name.into());
            }
        }

        Err(SelectCertError::RETRY)
    });
    Ok(builder.build().into_context())
}

pub(crate) struct OpensslHost {
    pub(super) config: Arc<OpensslHostConfig>,
    pub(super) ssl_context: Option<SslContext>,
    #[cfg(feature = "vendored-tongsuo")]
    pub(super) tlcp_context: Option<SslContext>,
    req_alive_sem: Option<GaugeSemaphore>,
    request_rate_limit: Option<Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>>,
    pub(crate) backends: Arc<ArcSwap<AlpnMatch<ArcBackend>>>,
}

impl OpensslHost {
    pub(super) fn try_build(config: &Arc<OpensslHostConfig>) -> anyhow::Result<Self> {
        let ssl_context = config.build_ssl_context()?;
        #[cfg(feature = "vendored-tongsuo")]
        let tlcp_context = config.build_tlcp_context()?;

        let backends = config.backends.build(crate::backend::get_or_insert_default);

        let request_rate_limit = config
            .request_rate_limit
            .as_ref()
            .map(|quota| Arc::new(RateLimiter::direct(quota.get_inner())));
        let req_alive_sem = config.request_alive_max.map(GaugeSemaphore::new);

        Ok(OpensslHost {
            config: config.clone(),
            ssl_context,
            #[cfg(feature = "vendored-tongsuo")]
            tlcp_context,
            req_alive_sem,
            request_rate_limit,
            backends: Arc::new(ArcSwap::new(Arc::new(backends))),
        })
    }

    pub(super) fn new_for_reload(&self, config: Arc<OpensslHostConfig>) -> anyhow::Result<Self> {
        let ssl_context = config.build_ssl_context()?;
        #[cfg(feature = "vendored-tongsuo")]
        let tlcp_context = config.build_tlcp_context()?;

        let request_rate_limit = if let Some(quota) = &config.request_rate_limit {
            if let Some(old_limiter) = &self.request_rate_limit {
                if let Some(old_quota) = &self.config.request_rate_limit {
                    if quota.eq(old_quota) {
                        // always use the old rate limiter when possible
                        Some(Arc::clone(old_limiter))
                    } else {
                        Some(Arc::new(RateLimiter::direct(quota.get_inner())))
                    }
                } else {
                    unreachable!()
                }
            } else {
                Some(Arc::new(RateLimiter::direct(quota.get_inner())))
            }
        } else {
            None
        };
        let req_alive_sem = if let Some(p) = &config.request_alive_max {
            let sema = self
                .req_alive_sem
                .as_ref()
                .map(|sema| sema.new_updated(*p))
                .unwrap_or_else(|| GaugeSemaphore::new(*p));
            Some(sema)
        } else {
            None
        };

        let new_host = OpensslHost {
            config,
            ssl_context,
            #[cfg(feature = "vendored-tongsuo")]
            tlcp_context,
            req_alive_sem,
            request_rate_limit,
            backends: self.backends.clone(), // use the old container
        };
        new_host.update_backends(); // update backends using the new config
        Ok(new_host)
    }

    pub(super) fn check_rate_limit(&self) -> Result<(), ()> {
        if let Some(limit) = &self.request_rate_limit {
            if limit.check().is_err() {
                // TODO add stats
                return Err(());
            }
        }
        Ok(())
    }

    pub(super) fn acquire_request_semaphore(&self) -> Result<Option<GaugeSemaphorePermit>, ()> {
        self.req_alive_sem
            .as_ref()
            .map(|sem| sem.try_acquire().map_err(|_| {}))
            .transpose()
    }

    pub(super) fn get_backend(&self, protocol: &str) -> Option<ArcBackend> {
        self.backends.load().get(protocol).cloned()
    }

    pub(super) fn get_default_backend(&self) -> Option<ArcBackend> {
        self.backends.load().get_default().cloned()
    }

    pub(super) fn use_backend(&self, name: &MetricsName) -> bool {
        self.config.backends.contains_value(name)
    }

    pub(super) fn update_backends(&self) {
        let backends = self
            .config
            .backends
            .build(crate::backend::get_or_insert_default);
        self.backends.store(Arc::new(backends));
    }
}

impl NamedValue for OpensslHost {
    type Name = str;
    type NameOwned = String;

    fn name(&self) -> &Self::Name {
        self.config.name()
    }

    fn name_owned(&self) -> Self::NameOwned {
        self.config.name_owned()
    }
}
