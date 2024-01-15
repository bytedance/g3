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

use g3_ctl::CommandResult;

use g3keymess_proto::proc_capnp::proc_control;
use g3keymess_proto::server_capnp::server_control;

use crate::common::parse_operation_result;

pub const COMMAND: &str = "server";

const COMMAND_ARG_NAME: &str = "server";

const SUBCOMMAND_STATUS: &str = "status";
const SUBCOMMAND_ADD_METRICS_TAG: &str = "add-metrics-tag";
const SUBCOMMAND_GET_LISTEN_ADDR: &str = "get-listen-addr";

const SUBCOMMAND_ARG_NAME: &str = "name";
const SUBCOMMAND_ARG_VALUE: &str = "value";

pub fn command() -> Command {
    Command::new(COMMAND)
        .arg(Arg::new(COMMAND_ARG_NAME).required(true).num_args(1))
        .subcommand_required(true)
        .subcommand(Command::new(SUBCOMMAND_STATUS))
        .subcommand(
            Command::new(SUBCOMMAND_ADD_METRICS_TAG)
                .arg(
                    Arg::new(SUBCOMMAND_ARG_NAME)
                        .help("Tag name")
                        .required(true)
                        .long(SUBCOMMAND_ARG_NAME)
                        .num_args(1),
                )
                .arg(
                    Arg::new(SUBCOMMAND_ARG_VALUE)
                        .help("Tag value")
                        .required(true)
                        .long(SUBCOMMAND_ARG_VALUE)
                        .num_args(1),
                ),
        )
        .subcommand(Command::new(SUBCOMMAND_GET_LISTEN_ADDR))
}

async fn status(client: &server_control::Client) -> CommandResult<()> {
    let req = client.status_request();
    let rsp = req.send().promise.await?;
    let stats = rsp.get()?.get_status()?;
    println!("online: {}", stats.get_online());
    println!("alive tasks: {}", stats.get_alive_task_count());
    println!("total task: {}", stats.get_total_task_count());
    Ok(())
}

async fn add_metrics_tag(client: &server_control::Client, args: &ArgMatches) -> CommandResult<()> {
    let name = args.get_one::<String>(SUBCOMMAND_ARG_NAME).unwrap();
    let value = args.get_one::<String>(SUBCOMMAND_ARG_VALUE).unwrap();

    let mut req = client.add_metrics_tag_request();
    req.get().set_name(name);
    req.get().set_value(value);
    let rsp = req.send().promise.await?;
    parse_operation_result(rsp.get()?.get_result()?)
}

async fn get_listen_addr(client: &server_control::Client) -> CommandResult<()> {
    let req = client.get_listen_addr_request();
    let rsp = req.send().promise.await?;
    g3_ctl::print_text("addr", rsp.get()?.get_addr()?)
}

pub async fn run(client: &proc_control::Client, args: &ArgMatches) -> CommandResult<()> {
    let name = args.get_one::<String>(COMMAND_ARG_NAME).unwrap();

    let (subcommand, sub_args) = args.subcommand().unwrap();
    match subcommand {
        SUBCOMMAND_STATUS => {
            super::proc::get_server(client, name)
                .and_then(|server| async move { status(&server).await })
                .await
        }
        SUBCOMMAND_ADD_METRICS_TAG => {
            super::proc::get_server(client, name)
                .and_then(|server| async move { add_metrics_tag(&server, sub_args).await })
                .await
        }
        SUBCOMMAND_GET_LISTEN_ADDR => {
            super::proc::get_server(client, name)
                .and_then(|server| async move { get_listen_addr(&server).await })
                .await
        }
        _ => unreachable!(),
    }
}
