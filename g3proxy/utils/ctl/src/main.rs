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

use std::io;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::anyhow;
use capnp_rpc::{rpc_twoparty_capnp, twoparty, RpcSystem};
use clap::builder::ArgPredicate;
use clap::{value_parser, Arg, ArgMatches, Command, ValueHint};
use clap_complete::Shell;
use tokio::io::AsyncWriteExt;
use tokio::net::UnixStream;

use g3proxy_proto::proc_capnp::proc_control;

mod common;
mod error;
mod proc;

mod escaper;
mod resolver;
mod server;
mod user_group;

use error::{CommandError, CommandResult};

const DEFAULT_SYS_CONTROL_DIR: &str = "/run/g3proxy";
const DEFAULT_TMP_CONTROL_DIR: &str = "/tmp/g3";

const GLOBAL_ARG_COMPLETION: &str = "completion";
const GLOBAL_ARG_CONTROL_DIR: &str = "control-dir";
const GLOBAL_ARG_GROUP: &str = "daemon-group";
const GLOBAL_ARG_PID: &str = "pid";

async fn connect_to_daemon(args: &ArgMatches) -> anyhow::Result<UnixStream> {
    let control_dir = args.get_one::<PathBuf>(GLOBAL_ARG_CONTROL_DIR).unwrap();
    let daemon_group = args
        .get_one::<String>(GLOBAL_ARG_GROUP)
        .map(|s| s.as_str())
        .unwrap_or_default();

    let socket_path = match args.get_one::<usize>(GLOBAL_ARG_PID) {
        Some(pid) => control_dir.join(format!("{daemon_group}_{}.sock", *pid)),
        None => control_dir.join(format!("{daemon_group}.sock")),
    };

    let mut stream = tokio::net::UnixStream::connect(&socket_path)
        .await
        .map_err(|e| {
            anyhow!(
                "failed to connect to control socket {}: {e:?}",
                socket_path.display()
            )
        })?;
    stream
        .write_all(b"capnp\n")
        .await
        .map_err(|e| anyhow!("enter capnp mode failed: {e:?}"))?;
    stream
        .flush()
        .await
        .map_err(|e| anyhow!("enter capnp mod failed: {e:?}"))?;
    Ok(stream)
}

fn dir_exist(dir: &str) -> bool {
    let path = PathBuf::from_str(dir).unwrap();
    std::fs::read_dir(path).is_ok()
}

fn auto_detect_control_dir() -> &'static str {
    if dir_exist(DEFAULT_SYS_CONTROL_DIR) {
        DEFAULT_SYS_CONTROL_DIR
    } else {
        DEFAULT_TMP_CONTROL_DIR
    }
}

fn build_cli_args() -> Command {
    Command::new("g3proxy-ctl")
        .arg(
            Arg::new(GLOBAL_ARG_COMPLETION)
                .num_args(1)
                .value_name("SHELL")
                .long("completion")
                .value_parser(value_parser!(Shell))
                .exclusive(true),
        )
        .arg(
            Arg::new(GLOBAL_ARG_CONTROL_DIR)
                .help("Directory that contains the control socket")
                .value_name("CONTROL DIR")
                .value_hint(ValueHint::DirPath)
                .value_parser(value_parser!(PathBuf))
                .short('C')
                .long("control-dir")
                .default_value(auto_detect_control_dir())
                .default_value_if(GLOBAL_ARG_COMPLETION, ArgPredicate::IsPresent, None),
        )
        .arg(
            Arg::new(GLOBAL_ARG_GROUP)
                .required_unless_present_any([GLOBAL_ARG_PID, GLOBAL_ARG_COMPLETION])
                .num_args(1)
                .value_name("GROUP NAME")
                .help("Daemon group name")
                .short('G')
                .long("daemon-group"),
        )
        .arg(
            Arg::new(GLOBAL_ARG_PID)
                .help("Daemon pid")
                .required_unless_present_any([GLOBAL_ARG_GROUP, GLOBAL_ARG_COMPLETION])
                .num_args(1)
                .value_name("PID")
                .value_parser(value_parser!(usize))
                .short('p')
                .long("daemon-pid"),
        )
        .subcommand_required(true)
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

    if let Some(target) = args.get_one::<Shell>(GLOBAL_ARG_COMPLETION) {
        let mut app = build_cli_args();
        let bin_name = app.get_name().to_string();
        clap_complete::generate(*target, &mut app, bin_name, &mut io::stdout());
        return Ok(());
    }

    let stream = connect_to_daemon(&args).await?;

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

            if let Some((subcommand, args)) = args.subcommand() {
                match subcommand {
                    proc::COMMAND_VERSION => proc::version(&proc_control).await,
                    proc::COMMAND_OFFLINE => proc::offline(&proc_control).await,
                    proc::COMMAND_FORCE_QUIT => proc::force_quit(&proc_control, args).await,
                    proc::COMMAND_FORCE_QUIT_ALL => proc::force_quit_all(&proc_control).await,
                    proc::COMMAND_LIST => proc::list(&proc_control, args).await,
                    proc::COMMAND_RELOAD_USER_GROUP => {
                        proc::reload_user_group(&proc_control, args).await
                    }
                    proc::COMMAND_RELOAD_RESOLVER => {
                        proc::reload_resolver(&proc_control, args).await
                    }
                    proc::COMMAND_RELOAD_AUDITOR => proc::reload_auditor(&proc_control, args).await,
                    proc::COMMAND_RELOAD_ESCAPER => proc::reload_escaper(&proc_control, args).await,
                    proc::COMMAND_RELOAD_SERVER => proc::reload_server(&proc_control, args).await,
                    user_group::COMMAND => user_group::run(&proc_control, args).await,
                    resolver::COMMAND => resolver::run(&proc_control, args).await,
                    escaper::COMMAND => escaper::run(&proc_control, args).await,
                    server::COMMAND => server::run(&proc_control, args).await,
                    cmd => Err(CommandError::Cli(format!("invalid subcommand {cmd}"))),
                }
            } else {
                Err(CommandError::Cli("no subcommand found".to_string()))
            }
        })
        .await
        .map_err(anyhow::Error::new)
}
