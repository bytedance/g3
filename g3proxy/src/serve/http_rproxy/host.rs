/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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
