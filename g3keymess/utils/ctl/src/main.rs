/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use clap::Command;

use g3_ctl::{CommandError, DaemonCtlArgs, DaemonCtlArgsExt};

use g3keymess_proto::proc_capnp::proc_control;

mod common;
mod proc;

mod server;

mod local;

fn build_cli_args() -> Command {
    Command::new(env!("CARGO_PKG_NAME"))
        .append_daemon_ctl_args()
        .subcommand(proc::commands::version())
        .subcommand(proc::commands::offline())
        .subcommand(proc::commands::cancel_shutdown())
        .subcommand(proc::commands::list())
        .subcommand(proc::commands::publish_key())
        .subcommand(proc::commands::check_key())
        .subcommand(server::command())
        .subcommand(local::commands::check_dup())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args = build_cli_args().get_matches();

    let mut ctl_opts = DaemonCtlArgs::parse_clap(&args);
    if ctl_opts.generate_shell_completion(build_cli_args) {
        return Ok(());
    }

    let (rpc_system, proc_control) = ctl_opts
        .connect_rpc::<proc_control::Client>("g3keymess")
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
                proc::COMMAND_PUBLISH_KEY => proc::publish_key(&proc_control, args).await,
                proc::COMMAND_CHECK_KEY => proc::check_key(&proc_control, args).await,
                server::COMMAND => server::run(&proc_control, args).await,
                local::COMMAND_CHECK_DUP => local::check_dup(args),
                _ => Err(CommandError::Cli(anyhow!(
                    "unsupported command {subcommand}"
                ))),
            }
        })
        .await
        .map_err(anyhow::Error::new)
}
