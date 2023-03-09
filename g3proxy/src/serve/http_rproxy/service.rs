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

use anyhow::Context;

use g3_types::net::OpensslTlsClientConfig;

use crate::config::server::http_rproxy::HttpServiceConfig;

pub(crate) struct HttpService {
    pub(super) config: Arc<HttpServiceConfig>,
    pub(super) tls_client: Option<OpensslTlsClientConfig>,
}

impl TryFrom<&Arc<HttpServiceConfig>> for HttpService {
    type Error = anyhow::Error;

    fn try_from(value: &Arc<HttpServiceConfig>) -> Result<Self, Self::Error> {
        HttpService::build(value)
    }
}

impl HttpService {
    fn build(config: &Arc<HttpServiceConfig>) -> anyhow::Result<Self> {
        let tls_client = if let Some(builder) = &config.tls_client_builder {
            let client = builder.build().context("failed to build tls client")?;
            Some(client)
        } else {
            None
        };

        Ok(HttpService {
            config: Arc::clone(config),
            tls_client,
        })
    }
}
