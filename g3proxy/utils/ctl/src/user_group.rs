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

use g3proxy_proto::proc_capnp::proc_control;
use g3proxy_proto::user_group_capnp::user_group_control;

use super::common::print_list_text;
use super::{CommandError, CommandResult};

pub const COMMAND: &str = "user-group";

const COMMAND_ARG_NAME: &str = "name";

const SUBCOMMAND_LIST_STATIC_USER: &str = "list-static-user";
const SUBCOMMAND_LIST_DYNAMIC_USER: &str = "list-dynamic-user";

pub fn command() -> Command {
    Command::new(COMMAND)
        .arg(Arg::new(COMMAND_ARG_NAME).required(true).num_args(1))
        .subcommand(Command::new(SUBCOMMAND_LIST_STATIC_USER))
        .subcommand(Command::new(SUBCOMMAND_LIST_DYNAMIC_USER))
}

pub async fn run(client: &proc_control::Client, args: &ArgMatches) -> CommandResult<()> {
    let name = args.get_one::<String>(COMMAND_ARG_NAME).unwrap();

    let user_group = super::proc::get_user_group(client, name).await?;

    if let Some((subcommand, _)) = args.subcommand() {
        match subcommand {
            SUBCOMMAND_LIST_STATIC_USER => list_static_user(&user_group).await,
            SUBCOMMAND_LIST_DYNAMIC_USER => list_dynamic_user(&user_group).await,
            cmd => Err(CommandError::Cli(format!("unsupported subcommand {cmd}"))),
        }
    } else {
        Ok(())
    }
}

async fn list_static_user(client: &user_group_control::Client) -> CommandResult<()> {
    let req = client.list_static_user_request();
    let rsp = req.send().promise.await?;
    print_list_text(rsp.get()?.get_result()?)
}

async fn list_dynamic_user(client: &user_group_control::Client) -> CommandResult<()> {
    let req = client.list_dynamic_user_request();
    let rsp = req.send().promise.await?;
    print_list_text(rsp.get()?.get_result()?)
}
