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

use capnp::capability::Promise;

use g3_types::metrics::MetricsName;

use g3proxy_proto::server_capnp::server_control;

use crate::serve::ArcServer;

pub(super) struct ServerControlImpl {
    server: ArcServer,
}

impl ServerControlImpl {
    pub(super) fn new_client(name: &str) -> anyhow::Result<server_control::Client> {
        let name = unsafe { MetricsName::from_str_unchecked(name) };
        let server = crate::serve::get_server(&name)?;
        Ok(capnp_rpc::new_client(ServerControlImpl { server }))
    }
}

impl server_control::Server for ServerControlImpl {
    fn status(
        &mut self,
        _params: server_control::StatusParams,
        mut results: server_control::StatusResults,
    ) -> Promise<(), capnp::Error> {
        if let Some(stats) = self.server.get_server_stats() {
            let mut builder = results.get().init_status();
            builder.set_online(stats.is_online());
            builder.set_alive_task_count(stats.get_alive_count());
            builder.set_total_conn_count(stats.get_conn_total());
            builder.set_total_task_count(stats.get_task_total());
            Promise::ok(())
        } else {
            Promise::err(capnp::Error::failed(
                "server status is not supported on this server".to_string(),
            ))
        }
    }
}
