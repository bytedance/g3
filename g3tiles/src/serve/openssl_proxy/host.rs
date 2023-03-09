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
use g3_types::net::Host;
use g3_types::route::{AlpnMatch, HostMatch};

use super::OpensslService;
use crate::config::server::openssl_proxy::OpensslHostConfig;

pub(super) fn build_ssl_acceptor(
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

            if let Err(e) = ssl.set_ssl_context(&host.ssl_context) {
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

pub(crate) struct OpensslHost {
    pub(super) config: Arc<OpensslHostConfig>,
    ssl_context: SslContext,
    req_alive_sem: Option<GaugeSemaphore>,
    request_rate_limit: Option<Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>>,
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
        let ssl_context = config.build_ssl_context()?;

        let services = (&config.services).try_into()?;

        let request_rate_limit = config
            .request_rate_limit
            .as_ref()
            .map(|quota| Arc::new(RateLimiter::direct(quota.get_inner())));
        let req_alive_sem = config.request_alive_max.map(GaugeSemaphore::new);

        Ok(OpensslHost {
            config,
            ssl_context,
            req_alive_sem,
            request_rate_limit,
            services,
        })
    }

    pub(super) fn new_for_reload(&self, config: Arc<OpensslHostConfig>) -> anyhow::Result<Self> {
        let ssl_context = config.build_ssl_context()?;

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

        Ok(OpensslHost {
            config,
            ssl_context,
            req_alive_sem,
            request_rate_limit,
            services,
        })
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
