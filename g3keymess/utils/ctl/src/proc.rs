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

use g3keymess_proto::proc_capnp::proc_control;
use g3keymess_proto::server_capnp::server_control;

use super::CommandResult;
use crate::common::{parse_fetch_result, parse_operation_result, print_list_text};

pub const COMMAND_VERSION: &str = "version";
pub const COMMAND_OFFLINE: &str = "offline";

pub const COMMAND_LIST: &str = "list";

const COMMAND_LIST_ARG_RESOURCE: &str = "resource";
const RESOURCE_VALUE_SERVER: &str = "server";

pub mod commands {
    use super::*;
    use clap::{Arg, Command};

    pub fn version() -> Command {
        Command::new(COMMAND_VERSION)
    }

    pub fn offline() -> Command {
        Command::new(COMMAND_OFFLINE).about("Put this daemon into offline mode")
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

pub(crate) async fn get_server(
    client: &proc_control::Client,
    name: &str,
) -> CommandResult<server_control::Client> {
    let mut req = client.get_server_request();
    req.get().set_name(name);
    let rsp = req.send().promise.await?;
    parse_fetch_result(rsp.get()?.get_server()?)
}
