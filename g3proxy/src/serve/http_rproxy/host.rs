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

use g3_types::net::{OpensslClientConfig, OpensslTicketKey, RollingTicketer, RustlsServerConfig};

use crate::config::server::http_rproxy::HttpHostConfig;

pub(crate) struct HttpHost {
    pub(super) config: Arc<HttpHostConfig>,
    pub(super) tls_server: Option<RustlsServerConfig>,
    pub(super) tls_client: Option<OpensslClientConfig>,
}

impl HttpHost {
    pub(super) fn try_build(
        config: &Arc<HttpHostConfig>,
        ticketer: Option<Arc<RollingTicketer<OpensslTicketKey>>>,
    ) -> anyhow::Result<Self> {
        let tls_server = if let Some(builder) = &config.tls_server_builder {
            let server = builder
                .build_with_ticketer(ticketer)
                .context("failed to build tls server")?;
            Some(server)
        } else {
            None
        };

        let tls_client = if let Some(builder) = &config.tls_client_builder {
            let client = builder.build().context("failed to build tls client")?;
            Some(client)
        } else {
            None
        };

        Ok(HttpHost {
            config: Arc::clone(config),
            tls_server,
            tls_client,
        })
    }
}
