/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use clap::{Arg, ArgMatches, Command};
use tokio::io::{AsyncRead, AsyncWrite};

use g3_ftp_client::{FtpClient, FtpConnectionProvider};

pub(super) const COMMAND: &str = "stat";

const COMMAND_ARG_PATH: &str = "path";

pub(super) fn command() -> Command {
    Command::new(COMMAND)
        .about("Fetch file stats")
        .arg(Arg::new(COMMAND_ARG_PATH).value_name("PATH").num_args(1))
}

pub(super) async fn run<CP, S, E, UD>(
    client: &mut FtpClient<CP, S, E, UD>,
    args: &ArgMatches,
) -> anyhow::Result<()>
where
    CP: FtpConnectionProvider<S, E, UD>,
    S: AsyncRead + AsyncWrite + Unpin,
    E: std::error::Error,
{
    let path = args
        .get_one::<String>(COMMAND_ARG_PATH)
        .map(|s| s.as_str())
        .unwrap_or_default();

    let facts = client.fetch_file_facts(path).await?;

    println!("Path: {}", facts.entry_path());
    println!("Type: {}", facts.entry_type());
    if let Some(size) = facts.size() {
        println!("Size: {size}");
    }
    if let Some(dt) = facts.mtime() {
        println!("Modify Time: {dt}");
    }

    Ok(())
}
