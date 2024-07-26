/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use g3_daemon::control::upgrade::UpgradeAction;

use anyhow::anyhow;
use capnp_rpc::rpc_twoparty_capnp::Side;
use capnp_rpc::RpcSystem;

use g3keymess_proto::proc_capnp::proc_control;
use g3keymess_proto::types_capnp::operation_result;

use g3_daemon::control::LocalController;

pub struct UpgradeActor {
    proc_control: proc_control::Client,
}

impl UpgradeAction for UpgradeActor {
    async fn connect_rpc() -> anyhow::Result<(RpcSystem<Side>, Self)> {
        LocalController::connect_rpc::<proc_control::Client>(
            crate::build::PKG_NAME,
            crate::opts::daemon_group(),
        )
        .await
        .map(|(r, proc_control)| (r, UpgradeActor { proc_control }))
    }

    async fn cancel_shutdown(&self) -> anyhow::Result<()> {
        let req = self.proc_control.cancel_shutdown_request();
        let rsp = req.send().promise.await?;
        check_operation_result(rsp.get()?.get_result()?)
    }

    async fn release_controller(&self) -> anyhow::Result<()> {
        let req = self.proc_control.release_controller_request();
        let rsp = req.send().promise.await?;
        check_operation_result(rsp.get()?.get_result()?)
    }

    async fn confirm_shutdown(&self) -> anyhow::Result<()> {
        let req = self.proc_control.offline_request();
        let rsp = req.send().promise.await?;
        check_operation_result(rsp.get()?.get_result()?)
    }
}

fn check_operation_result(r: operation_result::Reader<'_>) -> anyhow::Result<()> {
    match r.which().unwrap() {
        operation_result::Which::Ok(_) => Ok(()),
        operation_result::Which::Err(err) => {
            let e = err?;
            let msg = e.get_reason()?.to_str()?;
            Err(anyhow!("remote error: {} - {msg}", e.get_code()))
        }
    }
}
