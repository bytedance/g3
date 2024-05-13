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

use anyhow::anyhow;
use capnp_rpc::{rpc_twoparty_capnp, twoparty, RpcSystem};
use clap::Command;

use g3_ctl::{CommandError, DaemonCtlArgs, DaemonCtlArgsExt};

use g3proxy_proto::proc_capnp::proc_control;

mod common;
mod proc;

mod escaper;
mod resolver;
mod server;
mod user_group;

fn build_cli_args() -> Command {
    Command::new(env!("CARGO_PKG_NAME"))
        .append_daemon_ctl_args()
        .subcommand(proc::commands::version())
        .subcommand(proc::commands::offline())
        .subcommand(proc::commands::force_quit())
        .subcommand(proc::commands::force_quit_all())
        .subcommand(proc::commands::list())
        .subcommand(proc::commands::reload_user_group())
        .subcommand(proc::commands::reload_resolver())
        .subcommand(proc::commands::reload_auditor())
        .subcommand(proc::commands::reload_escaper())
        .subcommand(proc::commands::reload_server())
        .subcommand(user_group::command())
        .subcommand(resolver::command())
        .subcommand(escaper::command())
        .subcommand(server::command())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let args = build_cli_args().get_matches();

    let mut ctl_opts = DaemonCtlArgs::parse_clap(&args);
    if ctl_opts.generate_shell_completion(build_cli_args) {
        return Ok(());
    }

    let stream = ctl_opts.connect_to_daemon("g3proxy").await?;

    let (reader, writer) = tokio::io::split(stream);
    let reader = tokio_util::compat::TokioAsyncReadCompatExt::compat(reader);
    let writer = tokio_util::compat::TokioAsyncWriteCompatExt::compat_write(writer);
    let rpc_network = Box::new(twoparty::VatNetwork::new(
        reader,
        writer,
        rpc_twoparty_capnp::Side::Client,
        Default::default(),
    ));
    let mut rpc_system = RpcSystem::new(rpc_network, None);
    let proc_control: proc_control::Client = rpc_system.bootstrap(rpc_twoparty_capnp::Side::Server);

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
                proc::COMMAND_FORCE_QUIT => proc::force_quit(&proc_control, args).await,
                proc::COMMAND_FORCE_QUIT_ALL => proc::force_quit_all(&proc_control).await,
                proc::COMMAND_LIST => proc::list(&proc_control, args).await,
                proc::COMMAND_RELOAD_USER_GROUP => {
                    proc::reload_user_group(&proc_control, args).await
                }
                proc::COMMAND_RELOAD_RESOLVER => proc::reload_resolver(&proc_control, args).await,
                proc::COMMAND_RELOAD_AUDITOR => proc::reload_auditor(&proc_control, args).await,
                proc::COMMAND_RELOAD_ESCAPER => proc::reload_escaper(&proc_control, args).await,
                proc::COMMAND_RELOAD_SERVER => proc::reload_server(&proc_control, args).await,
                user_group::COMMAND => user_group::run(&proc_control, args).await,
                resolver::COMMAND => resolver::run(&proc_control, args).await,
                escaper::COMMAND => escaper::run(&proc_control, args).await,
                server::COMMAND => server::run(&proc_control, args).await,
                _ => Err(CommandError::Cli(anyhow!(
                    "unsupported command {subcommand}"
                ))),
            }
        })
        .await
        .map_err(anyhow::Error::new)
}
