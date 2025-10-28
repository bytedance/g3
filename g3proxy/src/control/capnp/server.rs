/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_types::metrics::NodeName;

use g3proxy_proto::server_capnp::server_control;

use crate::serve::ArcServer;

pub(super) struct ServerControlImpl {
    server: ArcServer,
}

impl ServerControlImpl {
    pub(super) fn new_client(name: &str) -> anyhow::Result<server_control::Client> {
        let name = unsafe { NodeName::new_unchecked(name) };
        let server = crate::serve::get_server(&name)?;
        Ok(capnp_rpc::new_client(ServerControlImpl { server }))
    }
}

impl server_control::Server for ServerControlImpl {
    async fn status(
        &self,
        _params: server_control::StatusParams,
        mut results: server_control::StatusResults,
    ) -> capnp::Result<()> {
        if let Some(stats) = self.server.get_server_stats() {
            let mut builder = results.get().init_status();
            builder.set_online(stats.is_online());
            builder.set_alive_task_count(stats.get_alive_count());
            builder.set_total_conn_count(stats.get_conn_total());
            builder.set_total_task_count(stats.get_task_total());
            Ok(())
        } else {
            Err(capnp::Error::failed(
                "server status is not supported on this server".to_string(),
            ))
        }
    }
}
