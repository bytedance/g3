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

use std::str::FromStr;
use std::sync::Arc;

use anyhow::anyhow;
use governor::{clock::DefaultClock, state::InMemoryState, state::NotKeyed, RateLimiter};
use log::debug;
use openssl::ex_data::Index;
use openssl::ssl::{NameType, SniError, Ssl, SslAcceptor, SslAlert, SslContext, SslMethod, SslRef};

use g3_types::collection::NamedValue;
use g3_types::limit::{GaugeSemaphore, GaugeSemaphorePermit};
use g3_types::net::{Host, OpensslServerSessionCache};
use g3_types::route::{AlpnMatch, HostMatch};

use super::OpensslService;
use crate::config::server::openssl_proxy::OpensslHostConfig;

#[cfg(feature = "vendored-tongsuo")]
const TLS_DEFAULT_CIPHER_SUITES: &str =
    "TLS_AES_128_GCM_SHA256:TLS_AES_256_GCM_SHA384:TLS_CHACHA20_POLY1305_SHA256:TLS_SM4_GCM_SM3";
#[cfg(feature = "vendored-tongsuo")]
const TLCP_DEFAULT_CIPHER_LIST: &str = "ECDHE-SM2-WITH-SM4-SM3:ECC-SM2-WITH-SM4-SM3:\
    ECDHE-SM2-SM4-CBC-SM3:ECDHE-SM2-SM4-GCM-SM3:ECC-SM2-SM4-CBC-SM3:ECC-SM2-SM4-GCM-SM3:\
    RSA-SM4-CBC-SM3:RSA-SM4-GCM-SM3:RSA-SM4-CBC-SHA256:RSA-SM4-GCM-SHA256";

pub(super) fn build_ssl_acceptor(
    worker_index: Index<Ssl, usize>,
    hosts: Arc<HostMatch<Arc<OpensslHost>>>,
    host_index: Index<Ssl, Arc<OpensslHost>>,
    sema_index: Index<Ssl, Option<GaugeSemaphorePermit>>,
    alert_unrecognized_name: bool,
) -> anyhow::Result<SslAcceptor> {
    let mut builder = SslAcceptor::mozilla_intermediate_v5(SslMethod::tls_server())
        .map_err(|e| anyhow!("failed to get ssl acceptor builder: {e}"))?;

    if openssl::version::number() < 0x101010a0 {
        // workaround bug https://github.com/openssl/openssl/issues/13291 to enable TLS1.3
        // which is fixed in
        //  Openssl 3.x: https://github.com/openssl/openssl/pull/13304
        //  Openssl 1.1.1j: https://github.com/openssl/openssl/pull/13305
        builder.set_psk_server_callback(|_ssl, _a, _b| Ok(0));
    }

    #[cfg(feature = "vendored-tongsuo")]
    builder
        .set_ciphersuites(TLS_DEFAULT_CIPHER_SUITES)
        .map_err(|e| anyhow!("failed to set tls1.3 cipher suites: {e}"))?;

    builder.set_servername_callback(move |ssl, alert| {
        let sni_err = if alert_unrecognized_name {
            *alert = SslAlert::UNRECOGNIZED_NAME;
            SniError::ALERT_FATAL
        } else {
            SniError::NOACK
        };

        let set_host_context = |ssl: &mut SslRef, host: &Arc<OpensslHost>| {
            if host.check_rate_limit().is_err() {
                return Err(sni_err);
            }
            // we do not check request alive sema here
            let Ok(sema) = host.acquire_request_semaphore() else {
                return Err(sni_err);
            };

            let worker_id = ssl.ex_data(worker_index).copied();
            let Some(ssl_context) = host.select_ssl_context(worker_id) else {
                return Err(sni_err);
            };

            if let Err(e) = ssl.set_ssl_context(ssl_context) {
                debug!("failed to set ssl context for host: {e}"); // TODO print host name
                Err(sni_err)
            } else {
                ssl.set_ex_data(host_index, host.clone());
                ssl.set_ex_data(sema_index, sema);
                Ok(())
            }
        };

        if let Some(sni) = ssl.servername(NameType::HOST_NAME) {
            match Host::from_str(sni) {
                Ok(name) => {
                    if let Some(host) = hosts.get(&name) {
                        return set_host_context(ssl, host);
                    }
                }
                Err(e) => {
                    debug!("invalid sni hostname: {e:?}");
                    return Err(sni_err);
                }
            }
        }

        if let Some(host) = hosts.get_default() {
            set_host_context(ssl, host)
        } else {
            Err(sni_err)
        }
    });

    Ok(builder.build())
}

#[cfg(feature = "vendored-tongsuo")]
pub(super) fn build_tlcp_context(
    worker_index: Index<Ssl, usize>,
    hosts: Arc<HostMatch<Arc<OpensslHost>>>,
    host_index: Index<Ssl, Arc<OpensslHost>>,
    sema_index: Index<Ssl, Option<GaugeSemaphorePermit>>,
    alert_unrecognized_name: bool,
) -> anyhow::Result<SslContext> {
    let mut builder = SslAcceptor::mozilla_intermediate_v5(SslMethod::ntls_server())
        .map_err(|e| anyhow!("failed to get ssl context builder: {e}"))?;
    builder.enable_force_ntls();

    builder
        .set_cipher_list(TLCP_DEFAULT_CIPHER_LIST)
        .map_err(|e| anyhow!("failed to set tlcp cipher list: {e}"))?;

    builder.set_servername_callback(move |ssl, alert| {
        let sni_err = if alert_unrecognized_name {
            *alert = SslAlert::UNRECOGNIZED_NAME;
            SniError::ALERT_FATAL
        } else {
            SniError::NOACK
        };

        let set_host_context = |ssl: &mut SslRef, host: &Arc<OpensslHost>| {
            if host.check_rate_limit().is_err() {
                return Err(sni_err);
            }
            // we do not check request alive sema here
            let Ok(sema) = host.acquire_request_semaphore() else {
                return Err(sni_err);
            };

            let worker_id = ssl.ex_data(worker_index).copied();
            let Some(ssl_context) = host.select_tlcp_context(worker_id) else {
                return Err(sni_err);
            };

            if let Err(e) = ssl.set_ssl_context(ssl_context) {
                debug!("failed to set tlcp ssl context for host: {e}"); // TODO print host name
                Err(sni_err)
            } else {
                ssl.set_ex_data(host_index, host.clone());
                ssl.set_ex_data(sema_index, sema);
                Ok(())
            }
        };

        if let Some(sni) = ssl.servername(NameType::HOST_NAME) {
            match Host::from_str(sni) {
                Ok(name) => {
                    if let Some(host) = hosts.get(&name) {
                        return set_host_context(ssl, host);
                    }
                }
                Err(e) => {
                    debug!("invalid sni hostname: {e:?}");
                    return Err(sni_err);
                }
            }
        }

        if let Some(host) = hosts.get_default() {
            set_host_context(ssl, host)
        } else {
            Err(sni_err)
        }
    });

    Ok(builder.build().into_context())
}

pub(crate) struct OpensslHost {
    pub(super) config: Arc<OpensslHostConfig>,
    ssl_context: Vec<SslContext>,
    #[cfg(feature = "vendored-tongsuo")]
    tlcp_context: Vec<SslContext>,
    req_alive_sem: Option<GaugeSemaphore>,
    request_rate_limit: Option<Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>>,
    session_cache: OpensslServerSessionCache,
    pub(crate) services: AlpnMatch<Arc<OpensslService>>,
}

impl TryFrom<&Arc<OpensslHostConfig>> for OpensslHost {
    type Error = anyhow::Error;

    fn try_from(value: &Arc<OpensslHostConfig>) -> Result<Self, Self::Error> {
        OpensslHost::build_new(value.clone())
    }
}

impl OpensslHost {
    pub(super) fn build_new(config: Arc<OpensslHostConfig>) -> anyhow::Result<Self> {
        let session_cache = OpensslServerSessionCache::new(config.session_cache_size)?;

        let services = (&config.services).try_into()?;

        let request_rate_limit = config
            .request_rate_limit
            .as_ref()
            .map(|quota| Arc::new(RateLimiter::direct(quota.get_inner())));
        let req_alive_sem = config.request_alive_max.map(GaugeSemaphore::new);

        let mut host = OpensslHost {
            config,
            ssl_context: Vec::new(),
            #[cfg(feature = "vendored-tongsuo")]
            tlcp_context: Vec::new(),
            req_alive_sem,
            request_rate_limit,
            session_cache,
            services,
        };
        host.set_ssl_context()?;
        #[cfg(feature = "vendored-tongsuo")]
        host.set_tlcp_context()?;
        Ok(host)
    }

    pub(super) fn new_for_reload(&self, config: Arc<OpensslHostConfig>) -> anyhow::Result<Self> {
        let services = (&config.services).try_into()?;

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

        let mut host = OpensslHost {
            config,
            ssl_context: Vec::new(),
            #[cfg(feature = "vendored-tongsuo")]
            tlcp_context: Vec::new(),
            req_alive_sem,
            request_rate_limit,
            session_cache: self.session_cache.clone(),
            services,
        };
        host.set_ssl_context()?;
        #[cfg(feature = "vendored-tongsuo")]
        host.set_tlcp_context()?;
        Ok(host)
    }

    fn set_ssl_context(&mut self) -> anyhow::Result<()> {
        let ctx_count = g3_daemon::runtime::worker::worker_count().max(1);
        let mut ssl_context = Vec::with_capacity(ctx_count);
        for _ in 0..ctx_count {
            if let Some(ctx) = self.config.build_ssl_context(&self.session_cache)? {
                ssl_context.push(ctx);
            }
        }
        self.ssl_context = ssl_context;
        Ok(())
    }

    fn select_ssl_context(&self, worker_id: Option<usize>) -> Option<&SslContext> {
        if let Some(id) = worker_id {
            self.ssl_context.get(id)
        } else {
            // TODO select random
            self.ssl_context.first()
        }
    }

    #[cfg(feature = "vendored-tongsuo")]
    fn set_tlcp_context(&mut self) -> anyhow::Result<()> {
        let ctx_count = g3_daemon::runtime::worker::worker_count().max(1);
        let mut tlcp_context = Vec::with_capacity(ctx_count);
        for _ in 0..ctx_count {
            if let Some(ctx) = self.config.build_tlcp_context(&self.session_cache)? {
                tlcp_context.push(ctx);
            }
        }
        self.tlcp_context = tlcp_context;
        Ok(())
    }

    #[cfg(feature = "vendored-tongsuo")]
    fn select_tlcp_context(&self, worker_id: Option<usize>) -> Option<&SslContext> {
        if let Some(id) = worker_id {
            self.tlcp_context.get(id)
        } else {
            // TODO select random
            self.tlcp_context.first()
        }
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
