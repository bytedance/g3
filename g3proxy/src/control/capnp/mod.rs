/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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
