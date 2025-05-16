/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use clap::ArgMatches;

use g3_ctl::CommandResult;

use g3statsd_proto::proc_capnp::proc_control;

use crate::common::parse_operation_result;

pub const COMMAND_VERSION: &str = "version";
pub const COMMAND_OFFLINE: &str = "offline";
pub const COMMAND_CANCEL_SHUTDOWN: &str = "cancel-shutdown";

pub const COMMAND_LIST: &str = "list";

const COMMAND_LIST_ARG_RESOURCE: &str = "resource";
const RESOURCE_VALUE_IMPORTER: &str = "importer";
const RESOURCE_VALUE_COLLECTOR: &str = "collector";
const RESOURCE_VALUE_EXPORTER: &str = "exporter";

pub const COMMAND_RELOAD_IMPORTER: &str = "reload-importer";
pub const COMMAND_RELOAD_COLLECTOR: &str = "reload-collector";
pub const COMMAND_RELOAD_EXPORTER: &str = "reload-exporter";

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

    pub fn cancel_shutdown() -> Command {
        Command::new(COMMAND_CANCEL_SHUTDOWN)
            .about("Cancel the shutdown progress if the daemon is still in shutdown wait state")
    }

    pub fn list() -> Command {
        Command::new(COMMAND_LIST).arg(
            Arg::new(COMMAND_LIST_ARG_RESOURCE)
                .required(true)
                .num_args(1)
                .value_parser([
                    RESOURCE_VALUE_IMPORTER,
                    RESOURCE_VALUE_COLLECTOR,
                    RESOURCE_VALUE_EXPORTER,
                ])
                .ignore_case(true),
        )
    }

    pub fn reload_importer() -> Command {
        Command::new(COMMAND_RELOAD_IMPORTER)
            .arg(Arg::new(SUBCOMMAND_ARG_NAME).required(true).num_args(1))
    }

    pub fn reload_collector() -> Command {
        Command::new(COMMAND_RELOAD_COLLECTOR)
            .arg(Arg::new(SUBCOMMAND_ARG_NAME).required(true).num_args(1))
    }

    pub fn reload_exporter() -> Command {
        Command::new(COMMAND_RELOAD_EXPORTER)
            .arg(Arg::new(SUBCOMMAND_ARG_NAME).required(true).num_args(1))
    }
}

pub async fn version(client: &proc_control::Client) -> CommandResult<()> {
    let req = client.version_request();
    let rsp = req.send().promise.await?;
    g3_ctl::print_version(rsp.get()?.get_version()?)
}

pub async fn offline(client: &proc_control::Client) -> CommandResult<()> {
    let req = client.offline_request();
    let rsp = req.send().promise.await?;
    parse_operation_result(rsp.get()?.get_result()?)
}

pub async fn cancel_shutdown(client: &proc_control::Client) -> CommandResult<()> {
    let req = client.cancel_shutdown_request();
    let rsp = req.send().promise.await?;
    parse_operation_result(rsp.get()?.get_result()?)
}

pub async fn list(client: &proc_control::Client, args: &ArgMatches) -> CommandResult<()> {
    match args
        .get_one::<String>(COMMAND_LIST_ARG_RESOURCE)
        .unwrap()
        .as_str()
    {
        RESOURCE_VALUE_IMPORTER => list_importer(client).await,
        RESOURCE_VALUE_COLLECTOR => list_collector(client).await,
        RESOURCE_VALUE_EXPORTER => list_exporter(client).await,
        _ => unreachable!(),
    }
}

async fn list_importer(client: &proc_control::Client) -> CommandResult<()> {
    let req = client.list_importer_request();
    let rsp = req.send().promise.await?;
    g3_ctl::print_result_list(rsp.get()?.get_result()?)
}

async fn list_collector(client: &proc_control::Client) -> CommandResult<()> {
    let req = client.list_collector_request();
    let rsp = req.send().promise.await?;
    g3_ctl::print_result_list(rsp.get()?.get_result()?)
}

async fn list_exporter(client: &proc_control::Client) -> CommandResult<()> {
    let req = client.list_exporter_request();
    let rsp = req.send().promise.await?;
    g3_ctl::print_result_list(rsp.get()?.get_result()?)
}

pub async fn reload_importer(
    client: &proc_control::Client,
    args: &ArgMatches,
) -> CommandResult<()> {
    let name = args.get_one::<String>(SUBCOMMAND_ARG_NAME).unwrap();
    let mut req = client.reload_importer_request();
    req.get().set_name(name);
    let rsp = req.send().promise.await?;
    parse_operation_result(rsp.get()?.get_result()?)
}

pub async fn reload_collector(
    client: &proc_control::Client,
    args: &ArgMatches,
) -> CommandResult<()> {
    let name = args.get_one::<String>(SUBCOMMAND_ARG_NAME).unwrap();
    let mut req = client.reload_collector_request();
    req.get().set_name(name);
    let rsp = req.send().promise.await?;
    parse_operation_result(rsp.get()?.get_result()?)
}

pub async fn reload_exporter(
    client: &proc_control::Client,
    args: &ArgMatches,
) -> CommandResult<()> {
    let name = args.get_one::<String>(SUBCOMMAND_ARG_NAME).unwrap();
    let mut req = client.reload_exporter_request();
    req.get().set_name(name);
    let rsp = req.send().promise.await?;
    parse_operation_result(rsp.get()?.get_result()?)
}
