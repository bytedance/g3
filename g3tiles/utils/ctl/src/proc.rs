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

use clap::ArgMatches;

use g3tiles_proto::proc_capnp::proc_control;
use g3tiles_proto::server_capnp::server_control;

use super::CommandResult;
use crate::common::{parse_fetch_result, parse_operation_result, print_list_text};

pub const COMMAND_VERSION: &str = "version";
pub const COMMAND_OFFLINE: &str = "offline";

pub const COMMAND_FORCE_QUIT: &str = "force-quit";
pub const COMMAND_FORCE_QUIT_ALL: &str = "force-quit-all";

pub const COMMAND_LIST: &str = "list";

const COMMAND_LIST_ARG_RESOURCE: &str = "resource";
const RESOURCE_VALUE_SERVER: &str = "server";

pub const COMMAND_RELOAD_SERVER: &str = "reload-server";

const SUBCOMMAND_ARG_NAME: &str = "name";

pub mod commands {
    use super::*;
    use clap::{Arg, Command};

    pub fn version() -> Command {
        Command::new(COMMAND_VERSION)
    }

    pub fn offline() -> Command {
        Command::new(COMMAND_OFFLINE).about("Put this daemon into offline mode")
    }

    pub fn force_quit() -> Command {
        Command::new(COMMAND_FORCE_QUIT)
            .about("Force quit offline server with the same name")
            .arg(Arg::new(SUBCOMMAND_ARG_NAME).required(true).num_args(1))
    }

    pub fn force_quit_all() -> Command {
        Command::new(COMMAND_FORCE_QUIT_ALL).about("Force quit all offline servers")
    }

    pub fn list() -> Command {
        Command::new(COMMAND_LIST).arg(
            Arg::new(COMMAND_LIST_ARG_RESOURCE)
                .required(true)
                .num_args(1)
                .value_parser([RESOURCE_VALUE_SERVER])
                .ignore_case(true),
        )
    }

    pub fn reload_server() -> Command {
        Command::new(COMMAND_RELOAD_SERVER)
            .arg(Arg::new(SUBCOMMAND_ARG_NAME).required(true).num_args(1))
    }
}

pub async fn version(client: &proc_control::Client) -> CommandResult<()> {
    let req = client.version_request();
    let rsp = req.send().promise.await?;
    let ver = rsp.get()?.get_version()?;
    println!("{ver}");
    Ok(())
}

pub async fn offline(client: &proc_control::Client) -> CommandResult<()> {
    let req = client.offline_request();
    let rsp = req.send().promise.await?;
    parse_operation_result(rsp.get()?.get_result()?)
}

pub async fn force_quit(client: &proc_control::Client, args: &ArgMatches) -> CommandResult<()> {
    let name = args.get_one::<String>(SUBCOMMAND_ARG_NAME).unwrap();
    let mut req = client.force_quit_offline_server_request();
    req.get().set_name(name);
    let rsp = req.send().promise.await?;
    parse_operation_result(rsp.get()?.get_result()?)
}

pub async fn force_quit_all(client: &proc_control::Client) -> CommandResult<()> {
    let req = client.force_quit_offline_servers_request();
    let rsp = req.send().promise.await?;
    parse_operation_result(rsp.get()?.get_result()?)
}

pub async fn list(client: &proc_control::Client, args: &ArgMatches) -> CommandResult<()> {
    match args
        .get_one::<String>(COMMAND_LIST_ARG_RESOURCE)
        .unwrap()
        .as_str()
    {
        RESOURCE_VALUE_SERVER => list_server(client).await,
        _ => unreachable!(),
    }
}

async fn list_server(client: &proc_control::Client) -> CommandResult<()> {
    let req = client.list_server_request();
    let rsp = req.send().promise.await?;
    print_list_text(rsp.get()?.get_result()?)
}

pub async fn reload_server(client: &proc_control::Client, args: &ArgMatches) -> CommandResult<()> {
    let name = args.get_one::<String>(SUBCOMMAND_ARG_NAME).unwrap();
    let mut req = client.reload_server_request();
    req.get().set_name(name);
    let rsp = req.send().promise.await?;
    parse_operation_result(rsp.get()?.get_result()?)
}

pub(crate) async fn get_server(
    client: &proc_control::Client,
    name: &str,
) -> CommandResult<server_control::Client> {
    let mut req = client.get_server_request();
    req.get().set_name(name);
    let rsp = req.send().promise.await?;
    parse_fetch_result(rsp.get()?.get_server()?)
}
