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

use openssl::x509::X509;

use g3_io_ext::EffectiveCacheHandle;
use g3_types::net::{TlsCertUsage, TlsServiceType};

use super::{CacheQueryKey, FakeCertPair};

pub struct CertAgentHandle {
    inner: EffectiveCacheHandle<CacheQueryKey, FakeCertPair>,
    request_timeout: Duration,
}

impl CertAgentHandle {
    pub(super) fn new(
        inner: EffectiveCacheHandle<CacheQueryKey, FakeCertPair>,
        request_timeout: Duration,
    ) -> Self {
        CertAgentHandle {
            inner,
            request_timeout,
        }
    }

    pub async fn pre_fetch(
        &self,
        service: TlsServiceType,
        usage: TlsCertUsage,
        host: Arc<str>,
    ) -> Option<FakeCertPair> {
        let query_key = CacheQueryKey::new(service, usage, host);
        self.inner
            .fetch_cache_only(Arc::new(query_key), self.request_timeout)
            .await
            .and_then(|r| r.inner().cloned())
    }

    pub async fn fetch(
        &self,
        service: TlsServiceType,
        usage: TlsCertUsage,
        host: Arc<str>,
        mimic_cert: X509,
    ) -> Option<FakeCertPair> {
        let mut query_key = CacheQueryKey::new(service, usage, host);
        query_key.set_mimic_cert(mimic_cert);
        self.inner
            .fetch(Arc::new(query_key), self.request_timeout)
            .await
            .and_then(|r| r.inner().cloned())
    }
}
