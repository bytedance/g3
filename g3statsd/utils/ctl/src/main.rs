/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use clap::Command;

use g3_ctl::{CommandError, DaemonCtlArgs, DaemonCtlArgsExt};

use g3statsd_proto::proc_capnp::proc_control;

mod common;
mod proc;

fn build_cli_args() -> Command {
    Command::new(env!("CARGO_PKG_NAME"))
        .append_daemon_ctl_args()
        .subcommand(proc::commands::version())
        .subcommand(proc::commands::offline())
        .subcommand(proc::commands::cancel_shutdown())
        .subcommand(proc::commands::list())
        .subcommand(proc::commands::reload_importer())
        .subcommand(proc::commands::reload_collector())
        .subcommand(proc::commands::reload_exporter())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args = build_cli_args().get_matches();

    let mut ctl_opts = DaemonCtlArgs::parse_clap(&args);
    if ctl_opts.generate_shell_completion(build_cli_args) {
        return Ok(());
    }

    let (rpc_system, proc_control) = ctl_opts
        .connect_rpc::<proc_control::Client>("g3statsd")
        .await?;

    tokio::task::LocalSet::new()
        .run_until(async move {
            tokio::task::spawn_local(async move {
                rpc_system
                    .await
                    .map_err(|e| eprintln!("rpc system error: {e:?}"))
            });

            let (subcommand, args) = args.subcommand().unwrap();
            match subcommand {
                proc::COMMAND_VERSION => proc::version(&proc_control).await,
                proc::COMMAND_OFFLINE => proc::offline(&proc_control).await,
                proc::COMMAND_CANCEL_SHUTDOWN => proc::cancel_shutdown(&proc_control).await,
                proc::COMMAND_LIST => proc::list(&proc_control, args).await,
                proc::COMMAND_RELOAD_IMPORTER => proc::reload_importer(&proc_control, args).await,
                proc::COMMAND_RELOAD_COLLECTOR => proc::reload_collector(&proc_control, args).await,
                proc::COMMAND_RELOAD_EXPORTER => proc::reload_exporter(&proc_control, args).await,
                _ => Err(CommandError::Cli(anyhow!(
                    "unsupported command {subcommand}"
                ))),
            }
        })
        .await
        .map_err(anyhow::Error::new)
}
