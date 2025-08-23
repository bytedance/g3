/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use g3_daemon::control::upgrade::UpgradeAction;

use anyhow::anyhow;
use capnp_rpc::RpcSystem;
use capnp_rpc::rpc_twoparty_capnp::Side;

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
