/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;
use std::time::Duration;

use arcstr::ArcStr;
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
        host: ArcStr,
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
        host: ArcStr,
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
