/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use clap::{Arg, ArgMatches, Command};
use tokio::io::{AsyncRead, AsyncWrite};

use g3_ftp_client::{FtpClient, FtpConnectionProvider};

pub(super) const COMMAND: &str = "rmd";

const COMMAND_ARG_PATH: &str = "path";

pub(super) fn command() -> Command {
    Command::new(COMMAND).about("Remove directory").arg(
        Arg::new(COMMAND_ARG_PATH)
            .value_name("DIRECTORY PATH")
            .num_args(1),
    )
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

    client.remove_dir(path).await?;

    Ok(())
}
