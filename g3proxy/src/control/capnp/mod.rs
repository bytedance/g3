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

use g3proxy_proto::proc_capnp::proc_control;

mod common;
use common::set_operation_result;
mod proc;

mod escaper;
mod resolver;
mod server;
mod user_group;

pub fn stop_working_thread() {
    g3_daemon::control::capnp::stop_working_thread();
}

fn build_capnp_client() -> capnp::capability::Client {
    let control_client: proc_control::Client = capnp_rpc::new_client(proc::ProcControlImpl);
    control_client.client
}

pub async fn spawn_working_thread() -> anyhow::Result<std::thread::JoinHandle<()>> {
    g3_daemon::control::capnp::spawn_working_thread(&build_capnp_client).await
}
