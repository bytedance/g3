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
use std::net::IpAddr;
use std::str::FromStr;
use std::sync::Arc;

use anyhow::anyhow;
use clap::{value_parser, Arg, ArgAction, Command};
use clap_complete::Shell;

use g3_ftp_client::{FtpClient, FtpClientConfig};
use g3_types::auth::{Password, Username};
use g3_types::net::UpstreamAddr;

mod connection;
mod logger;

mod cmd_del;
mod cmd_get;
mod cmd_list;
mod cmd_put;
mod cmd_rmd;
mod cmd_stat;

const GLOBAL_ARG_COMPLETION: &str = "completion";
const GLOBAL_ARG_SERVER: &str = "server";
const GLOBAL_ARG_USERNAME: &str = "username";
const GLOBAL_ARG_PASSWORD: &str = "password";
const GLOBAL_ARG_SOURCE_IP: &str = "source-ip";
const GLOBAL_ARG_VERBOSE: &str = "verbose";

fn build_cli_args() -> Command {
    Command::new("g3proxy-ftp")
        .arg(
            Arg::new(GLOBAL_ARG_COMPLETION)
                .num_args(1)
                .value_name("SHELL")
                .long("completion")
                .value_parser(value_parser!(Shell))
                .exclusive(true),
        )
        .arg(
            Arg::new(GLOBAL_ARG_SERVER)
                .help("FTP server address")
                .num_args(1)
                .value_name("SERVER ADDRESS")
                .required_unless_present(GLOBAL_ARG_COMPLETION),
        )
        .arg(
            Arg::new(GLOBAL_ARG_USERNAME)
                .help("FTP username")
                .num_args(1)
                .value_name("USERNAME")
                .short('u')
                .global(true),
        )
        .arg(
            Arg::new(GLOBAL_ARG_PASSWORD)
                .help("FTP password")
                .num_args(1)
                .value_name("PASSWORD")
                .short('p')
                .global(true),
        )
        .arg(
            Arg::new(GLOBAL_ARG_SOURCE_IP)
                .help("source ip address")
                .num_args(1)
                .value_name("IP ADDRESS")
                .value_parser(value_parser!(IpAddr))
                .long("source")
                .short('s')
                .global(true),
        )
        .arg(
            Arg::new(GLOBAL_ARG_VERBOSE)
                .help("show verbose message")
                .num_args(0)
                .action(ArgAction::Count)
                .short('v')
                .global(true),
        )
        .subcommand(cmd_list::command())
        .subcommand(cmd_stat::command())
        .subcommand(cmd_get::command())
        .subcommand(cmd_put::command())
        .subcommand(cmd_del::command())
        .subcommand(cmd_rmd::command())
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

    let verbose_level = args
        .get_one::<u8>(GLOBAL_ARG_VERBOSE)
        .copied()
        .unwrap_or_default();
    let logger = logger::SyncLogger::new(verbose_level);
    logger.into_global_logger().unwrap();

    let server = args.get_one::<String>(GLOBAL_ARG_SERVER).unwrap();
    let mut server = UpstreamAddr::from_str(server).unwrap();
    if server.port() == 0 {
        server.set_port(21);
    }

    let username = args
        .get_one::<String>(GLOBAL_ARG_USERNAME)
        .map(|s| Username::from_original(s).unwrap());
    let password = args
        .get_one::<String>(GLOBAL_ARG_PASSWORD)
        .map(|s| Password::from_original(s).unwrap());

    let mut conn_provider = connection::LocalConnectionProvider::default();
    if let Some(ip) = args.get_one::<IpAddr>(GLOBAL_ARG_SOURCE_IP) {
        conn_provider.set_bind_ip(*ip);
    }

    let config = Arc::new(FtpClientConfig::default());

    if let Some((subcommand, args)) = args.subcommand() {
        let mut client = match FtpClient::connect_to(server, conn_provider, &(), &config).await {
            Ok(client) => client,
            Err((e, _)) => return Err(e.into()),
        };
        client
            .new_user_session(username.as_ref(), password.as_ref())
            .await?;

        let ret = match subcommand {
            cmd_list::COMMAND => cmd_list::run(&mut client, args).await,
            cmd_stat::COMMAND => cmd_stat::run(&mut client, args).await,
            cmd_get::COMMAND => cmd_get::run(&mut client, args).await,
            cmd_put::COMMAND => cmd_put::run(&mut client, args).await,
            cmd_del::COMMAND => cmd_del::run(&mut client, args).await,
            cmd_rmd::COMMAND => cmd_rmd::run(&mut client, args).await,
            cmd => Err(anyhow!("invalid subcommand {cmd}")),
        };

        client.quit_and_close().await?;

        ret
    } else {
        Err(anyhow!("no subcommand found"))
    }
}
