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

use clap::{Arg, ArgMatches, Command};
use futures_util::future::TryFutureExt;

use g3tiles_proto::proc_capnp::proc_control;
use g3tiles_proto::server_capnp::server_control;

use super::{CommandError, CommandResult};

pub const COMMAND: &str = "server";

const COMMAND_ARG_NAME: &str = "name";

const SUBCOMMAND_STATUS: &str = "status";

pub fn command() -> Command {
    Command::new(COMMAND)
        .arg(Arg::new(COMMAND_ARG_NAME).required(true).num_args(1))
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

    if let Some((subcommand, _)) = args.subcommand() {
        match subcommand {
            SUBCOMMAND_STATUS => {
                super::proc::get_server(client, name)
                    .and_then(|server| async move { status(&server).await })
                    .await
            }
            cmd => Err(CommandError::Cli(format!("supported subcommand {cmd}"))),
        }
    } else {
        Ok(())
    }
}
