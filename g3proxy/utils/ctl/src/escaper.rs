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

use std::path::PathBuf;
use std::str::FromStr;

use clap::{value_parser, Arg, ArgMatches, Command, ValueHint};
use futures_util::future::TryFutureExt;

use g3proxy_proto::escaper_capnp::escaper_control;
use g3proxy_proto::proc_capnp::proc_control;

use super::{CommandError, CommandResult};
use crate::common::parse_operation_result;

pub const COMMAND: &str = "escaper";

const COMMAND_ARG_NAME: &str = "name";

const SUBCOMMAND_PUBLISH: &str = "publish";
const SUBCOMMAND_PUBLISH_ARG_FILE: &str = "file";
const SUBCOMMAND_PUBLISH_ARG_DATA: &str = "data";

pub fn command() -> Command {
    Command::new(COMMAND)
        .arg(Arg::new(COMMAND_ARG_NAME).required(true).num_args(1))
        .subcommand(
            Command::new(SUBCOMMAND_PUBLISH)
                .arg(
                    Arg::new(SUBCOMMAND_PUBLISH_ARG_FILE)
                        .value_name("FILE PATH")
                        .num_args(1)
                        .value_hint(ValueHint::FilePath)
                        .value_parser(value_parser!(PathBuf))
                        .required_unless_present(SUBCOMMAND_PUBLISH_ARG_DATA)
                        .conflicts_with(SUBCOMMAND_PUBLISH_ARG_DATA)
                        .short('f')
                        .long("file"),
                )
                .arg(
                    Arg::new(SUBCOMMAND_PUBLISH_ARG_DATA)
                        .value_name("JSON DATA")
                        .num_args(1)
                        .required_unless_present(SUBCOMMAND_PUBLISH_ARG_FILE)
                        .conflicts_with(SUBCOMMAND_PUBLISH_ARG_FILE),
                ),
        )
}

async fn publish(client: &escaper_control::Client, args: &ArgMatches) -> CommandResult<()> {
    let data = if let Some(file) = args.get_one::<PathBuf>(SUBCOMMAND_PUBLISH_ARG_FILE) {
        tokio::fs::read_to_string(file).await.map_err(|e| {
            CommandError::Cli(format!(
                "failed to read contents of file {}: {e:?}",
                file.display()
            ))
        })?
    } else if let Some(data) = args.get_one::<String>(SUBCOMMAND_PUBLISH_ARG_DATA) {
        data.to_string()
    } else {
        unreachable!()
    };

    if let Err(e) = serde_json::Value::from_str(&data) {
        return Err(CommandError::Cli(format!(
            "the data to publish is not valid json: {e:?}"
        )));
    }

    let mut req = client.publish_request();
    req.get().set_data(&data);
    let rsp = req.send().promise.await?;
    parse_operation_result(rsp.get()?.get_result()?)
}

pub async fn run(client: &proc_control::Client, args: &ArgMatches) -> CommandResult<()> {
    let name = args.get_one::<String>(COMMAND_ARG_NAME).unwrap();

    if let Some((subcommand, args)) = args.subcommand() {
        match subcommand {
            SUBCOMMAND_PUBLISH => {
                super::proc::get_escaper(client, name)
                    .and_then(|escaper| async move { publish(&escaper, args).await })
                    .await
            }
            cmd => Err(CommandError::Cli(format!("unsupported subcommand {cmd}"))),
        }
    } else {
        Ok(())
    }
}
