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

use std::str::FromStr;
use std::sync::Arc;

use anyhow::anyhow;
use capnp::capability::Promise;
use capnp_rpc::pry;

use g3_types::metrics::{MetricsName, MetricsTagName, MetricsTagValue};

use g3keymess_proto::server_capnp::server_control;

use super::set_operation_result;
use crate::serve::KeyServer;

pub(super) struct ServerControlImpl {
    server: Arc<KeyServer>,
}

impl ServerControlImpl {
    pub(super) fn new_client(name: &str) -> anyhow::Result<server_control::Client> {
        let name = unsafe { MetricsName::from_str_unchecked(name) };
        let server = crate::serve::get_server(&name)?;
        Ok(capnp_rpc::new_client(ServerControlImpl { server }))
    }

    fn do_add_metrics_tag(&self, name: &str, value: &str) -> anyhow::Result<()> {
        let name =
            MetricsTagName::from_str(name).map_err(|e| anyhow!("invalid metrics tag name: {e}"))?;
        let value = MetricsTagValue::from_str(value)
            .map_err(|e| anyhow!("invalid metrics tag value: {e}"))?;
        self.server.add_dynamic_metrics_tag(name, value);
        Ok(())
    }
}

impl server_control::Server for ServerControlImpl {
    fn status(
        &mut self,
        _params: server_control::StatusParams,
        mut results: server_control::StatusResults,
    ) -> Promise<(), capnp::Error> {
        let stats = self.server.get_server_stats();
        let mut builder = results.get().init_status();
        builder.set_online(stats.is_online());
        builder.set_alive_task_count(stats.get_alive_count());
        builder.set_total_task_count(stats.get_task_total());
        Promise::ok(())
    }

    fn add_metrics_tag(
        &mut self,
        params: server_control::AddMetricsTagParams,
        mut results: server_control::AddMetricsTagResults,
    ) -> Promise<(), capnp::Error> {
        let name = pry!(pry!(pry!(params.get()).get_name()).to_str());
        let value = pry!(pry!(pry!(params.get()).get_value()).to_str());

        let r = self.do_add_metrics_tag(name, value);
        set_operation_result(results.get().init_result(), r);
        Promise::ok(())
    }

    fn get_listen_addr(
        &mut self,
        _params: server_control::GetListenAddrParams,
        mut results: server_control::GetListenAddrResults,
    ) -> Promise<(), capnp::Error> {
        let addr = self.server.listen_addr().to_string();
        results.get().set_addr(addr.as_str());
        Promise::ok(())
    }
}
