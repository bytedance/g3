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

use rustls::{Certificate, PrivateKey};

use g3_io_ext::EffectiveCacheHandle;

use super::CacheQueryKey;

#[derive(Clone)]
pub struct CertAgentHandle {
    inner: EffectiveCacheHandle<CacheQueryKey, (Vec<Certificate>, PrivateKey)>,
    request_timeout: Duration,
}

impl CertAgentHandle {
    pub(crate) fn new(
        inner: EffectiveCacheHandle<CacheQueryKey, (Vec<Certificate>, PrivateKey)>,
        request_timeout: Duration,
    ) -> Self {
        CertAgentHandle {
            inner,
            request_timeout,
        }
    }

    pub async fn fetch(&self, host: String) -> Option<(Vec<Certificate>, PrivateKey)> {
        let query_key = CacheQueryKey { host };

        self.inner
            .fetch(Arc::new(query_key), self.request_timeout)
            .await
            .and_then(|r| r.inner().cloned())
    }
}
