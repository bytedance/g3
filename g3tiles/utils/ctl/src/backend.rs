/*
 * Copyright 2025 ByteDance and/or its affiliates.
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

use g3_ctl::CommandResult;

use g3tiles_proto::backend_capnp::backend_control;
use g3tiles_proto::proc_capnp::proc_control;

pub const COMMAND: &str = "backend";

const COMMAND_ARG_NAME: &str = "name";

const SUBCOMMAND_ALIVE_CONNECTION: &str = "alive-connection";

pub fn command() -> Command {
    Command::new(COMMAND)
        .arg(Arg::new(COMMAND_ARG_NAME).required(true).num_args(1))
        .subcommand_required(true)
        .subcommand(Command::new(SUBCOMMAND_ALIVE_CONNECTION))
}

async fn alive_connection(client: &backend_control::Client) -> CommandResult<()> {
    let req = client.alive_connection_request();
    let rsp = req.send().promise.await?;
    let count = rsp.get()?.get_count();
    println!("{count}");
    Ok(())
}

pub async fn run(client: &proc_control::Client, args: &ArgMatches) -> CommandResult<()> {
    let name = args.get_one::<String>(COMMAND_ARG_NAME).unwrap();

    let (subcommand, _) = args.subcommand().unwrap();
    match subcommand {
        SUBCOMMAND_ALIVE_CONNECTION => {
            super::proc::get_backend(client, name)
                .and_then(|backend| async move { alive_connection(&backend).await })
                .await
        }
        _ => unreachable!(),
    }
}
