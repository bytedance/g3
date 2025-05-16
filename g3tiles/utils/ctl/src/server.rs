/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use clap::{Arg, ArgMatches, Command};
use futures_util::future::TryFutureExt;

use g3_ctl::CommandResult;

use g3tiles_proto::proc_capnp::proc_control;
use g3tiles_proto::server_capnp::server_control;

pub const COMMAND: &str = "server";

const COMMAND_ARG_NAME: &str = "name";

const SUBCOMMAND_STATUS: &str = "status";

pub fn command() -> Command {
    Command::new(COMMAND)
        .arg(Arg::new(COMMAND_ARG_NAME).required(true).num_args(1))
        .subcommand_required(true)
        .subcommand(Command::new(SUBCOMMAND_STATUS))
}

async fn status(client: &server_control::Client) -> CommandResult<()> {
    let req = client.status_request();
    let rsp = req.send().promise.await?;
    let stats = rsp.get()?.get_status()?;
    println!("online: {}", stats.get_online());
    println!("alive tasks: {}", stats.get_alive_task_count());
    println!("total conn: {}", stats.get_total_conn_count());
    println!("total task: {}", stats.get_total_task_count());
    Ok(())
}

pub async fn run(client: &proc_control::Client, args: &ArgMatches) -> CommandResult<()> {
    let name = args.get_one::<String>(COMMAND_ARG_NAME).unwrap();

    let (subcommand, _) = args.subcommand().unwrap();
    match subcommand {
        SUBCOMMAND_STATUS => {
            super::proc::get_server(client, name)
                .and_then(|server| async move { status(&server).await })
                .await
        }
        _ => unreachable!(),
    }
}
