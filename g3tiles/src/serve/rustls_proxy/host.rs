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

use governor::{clock::DefaultClock, state::InMemoryState, state::NotKeyed, RateLimiter};
use rustls::ServerConfig;

use g3_types::collection::NamedValue;
use g3_types::limit::{GaugeSemaphore, GaugeSemaphorePermit};
use g3_types::route::AlpnMatch;

use super::RustlsService;
use crate::config::server::rustls_proxy::RustlsHostConfig;

pub(crate) struct RustlsHost {
    pub(super) config: Arc<RustlsHostConfig>,
    pub(super) tls_config: Arc<ServerConfig>,
    req_alive_sem: Option<GaugeSemaphore>,
    request_rate_limit: Option<Arc<RateLimiter<NotKeyed, InMemoryState, DefaultClock>>>,
    pub(crate) services: AlpnMatch<Arc<RustlsService>>,
}

impl TryFrom<&Arc<RustlsHostConfig>> for RustlsHost {
    type Error = anyhow::Error;

    fn try_from(value: &Arc<RustlsHostConfig>) -> Result<Self, Self::Error> {
        RustlsHost::build_new(value.clone())
    }
}

impl RustlsHost {
    pub(super) fn build_new(config: Arc<RustlsHostConfig>) -> anyhow::Result<Self> {
        let tls_config = config.build_tls_config()?;

        let services = (&config.services).try_into()?;

        let request_rate_limit = config
            .request_rate_limit
            .as_ref()
            .map(|quota| Arc::new(RateLimiter::direct(quota.get_inner())));
        let req_alive_sem = config.request_alive_max.map(GaugeSemaphore::new);

        Ok(RustlsHost {
            config,
            tls_config,
            req_alive_sem,
            request_rate_limit,
            services,
        })
    }

    pub(super) fn new_for_reload(&self, config: Arc<RustlsHostConfig>) -> anyhow::Result<Self> {
        let tls_config = config.build_tls_config()?;

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

        Ok(RustlsHost {
            config,
            tls_config,
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

impl NamedValue for RustlsHost {
    type Name = str;
    type NameOwned = String;

    fn name(&self) -> &Self::Name {
        self.config.name()
    }

    fn name_owned(&self) -> Self::NameOwned {
        self.config.name_owned()
    }
}
