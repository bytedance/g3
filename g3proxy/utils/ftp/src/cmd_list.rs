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

use async_trait::async_trait;
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

#[async_trait]
impl FtpLineDataReceiver for StdioLineReceiver {
    async fn recv_line(&mut self, line: &str) {
        self.has_error = self.io.write_all(line.as_bytes()).await.is_err();
    }

    #[inline]
    fn should_return_early(&self) -> bool {
        self.has_error
    }
}
