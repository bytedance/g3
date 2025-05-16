/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use clap::{Arg, ArgMatches, Command};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, Stdout};

use g3_ftp_client::{FtpClient, FtpConnectionProvider, FtpLineDataReceiver};

pub(super) const COMMAND: &str = "list";

const COMMAND_ARG_PATH: &str = "path";

pub(super) fn command() -> Command {
    Command::new(COMMAND).about("List path").arg(
        Arg::new(COMMAND_ARG_PATH)
            .value_name("FILE PATH")
            .num_args(1),
    )
}

pub(super) async fn run<CP, S, E>(
    client: &mut FtpClient<CP, S, E, ()>,
    args: &ArgMatches,
) -> anyhow::Result<()>
where
    CP: FtpConnectionProvider<S, E, ()>,
    S: AsyncRead + AsyncWrite + Unpin,
    E: std::error::Error,
{
    let path = args
        .get_one::<String>(COMMAND_ARG_PATH)
        .map(|s| s.as_str())
        .unwrap_or_default();

    let mut line_receiver = StdioLineReceiver::default();
    let data_stream = client.list_directory_detailed_start(path, &()).await?;
    client
        .list_directory_detailed_receive(data_stream, &mut line_receiver)
        .await?;
    Ok(())
}

pub struct StdioLineReceiver {
    io: Stdout,
    has_error: bool,
}

impl Default for StdioLineReceiver {
    fn default() -> Self {
        StdioLineReceiver {
            io: tokio::io::stdout(),
            has_error: false,
        }
    }
}

impl FtpLineDataReceiver for StdioLineReceiver {
    async fn recv_line(&mut self, line: &str) {
        self.has_error = self.io.write_all(line.as_bytes()).await.is_err();
    }

    #[inline]
    fn should_return_early(&self) -> bool {
        self.has_error
    }
}
