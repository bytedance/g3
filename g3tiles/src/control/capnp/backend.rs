/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use g3_types::metrics::NodeName;

use g3tiles_proto::backend_capnp::backend_control;

use crate::backend::ArcBackend;

pub(super) struct BackendControlImpl {
    backend: ArcBackend,
}

impl BackendControlImpl {
    pub(super) fn new_client(name: &str) -> anyhow::Result<backend_control::Client> {
        let name = unsafe { NodeName::new_unchecked(name) };
        let backend = crate::backend::get_backend(&name)?;
        Ok(capnp_rpc::new_client(BackendControlImpl { backend }))
    }
}

impl backend_control::Server for BackendControlImpl {
    async fn alive_connection(
        &self,
        _params: backend_control::AliveConnectionParams,
        mut results: backend_control::AliveConnectionResults,
    ) -> capnp::Result<()> {
        let alive_count = self.backend.alive_connection();
        results.get().set_count(alive_count);
        Ok(())
    }
}
