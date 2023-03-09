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

use anyhow::{anyhow, Context};
use clap::{value_parser, Arg, ArgAction, ArgMatches, Command, ValueHint};
use tokio::fs::File;
use tokio::io::{AsyncRead, AsyncWrite};

use g3_ftp_client::{FtpClient, FtpConnectionProvider};

pub(super) const COMMAND: &str = "get";

const COMMAND_ARG_PATH: &str = "path";
const COMMAND_ARG_OFFSET: &str = "offset";
const COMMAND_ARG_OUTPUT: &str = "output";
const COMMAND_ARG_TRUNCATE: &str = "truncate";

pub(super) fn command() -> Command {
    Command::new(COMMAND)
        .about("Download file")
        .arg(
            Arg::new(COMMAND_ARG_PATH)
                .help("remote download path")
                .value_name("FILE PATH")
                .num_args(1)
                .required(true),
        )
        .arg(
            Arg::new(COMMAND_ARG_OFFSET)
                .help("file offset")
                .value_name("OFFSET BYTES")
                .long_help("if not set, the size of the existed file will be used")
                .num_args(1)
                .value_parser(value_parser!(u64))
                .long("offset"),
        )
        .arg(
            Arg::new(COMMAND_ARG_OUTPUT)
                .help("local save path, default to stdio")
                .value_name("OUTPUT FILE")
                .num_args(1)
                .value_hint(ValueHint::FilePath)
                .value_parser(value_parser!(PathBuf))
                .long("output")
                .short('o'),
        )
        .arg(
            Arg::new(COMMAND_ARG_TRUNCATE)
                .help("truncate local file if existed")
                .action(ArgAction::SetTrue)
                .long("truncate"),
        )
}

async fn retrieve_file<CP, S, E, R>(
    ftp_client: &mut FtpClient<CP, S, E, ()>,
    mut data_stream: S,
    mut receive_stream: R,
) -> anyhow::Result<()>
where
    CP: FtpConnectionProvider<S, E, ()>,
    S: AsyncRead + AsyncWrite + Unpin,
    E: std::error::Error,
    R: AsyncWrite + Unpin,
{
    tokio::pin! {
        let data_copy = tokio::io::copy(&mut data_stream, &mut receive_stream);
    }

    tokio::select! {
        biased;

        r = &mut data_copy => {
            ftp_client.wait_retrieve_end_reply()
                .await
                .context("failed to recv retrieve end reply")?;
            if let Err(e) = r {
                Err(anyhow!("copy data stream failed: {e}"))
            } else {
                Ok(())
            }
        }
        r = ftp_client.wait_control_read_ready() => {
            if let Err(e) = r {
                return Err(anyhow!("unexpected control channel error: {e}"));
            }
            ftp_client.wait_retrieve_end_reply()
                .await
                .context("failed to recv retrieve end reply")?;

            match tokio::time::timeout(ftp_client.transfer_end_wait_timeout(), data_copy).await {
                Ok(Ok(_)) => Ok(()),
                Ok(Err(e)) => Err(anyhow!("copy data stream failed: {e}")),
                Err(_) => Err(anyhow!("timeout to wait transfer end")),
            }
        }
    }
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
    let path = args.get_one::<String>(COMMAND_ARG_PATH).unwrap();

    let cmd_offset = args.get_one::<u64>(COMMAND_ARG_OFFSET).copied();

    match args.get_one::<PathBuf>(COMMAND_ARG_OUTPUT) {
        Some(file) => {
            let (file_stream, offset) = if args.get_flag(COMMAND_ARG_TRUNCATE) {
                let stream = File::create(file).await.context(format!(
                    "failed to open file {} for truncate mode",
                    file.display()
                ))?;
                (stream, cmd_offset)
            } else {
                match std::fs::metadata(file) {
                    Ok(f) => {
                        if !f.file_type().is_file() {
                            return Err(anyhow!("{} is not a regular file", file.display()));
                        }
                        let offset = cmd_offset.unwrap_or(f.len());
                        let stream = std::fs::OpenOptions::new()
                            .append(true)
                            .open(file)
                            .context(format!(
                                "failed to open file {} for append mode",
                                file.display()
                            ))?;
                        (File::from_std(stream), Some(offset))
                    }
                    Err(_) => {
                        // file not existed
                        let stream = File::create(file).await.context(format!(
                            "failed to open file {} for truncate mode",
                            file.display()
                        ))?;
                        (stream, cmd_offset)
                    }
                }
            };

            let (data_stream, _transfer_size) =
                client.retrieve_file_start(path, offset, &()).await?;
            retrieve_file(client, data_stream, file_stream).await
        }
        None => {
            let (data_stream, _transfer_size) =
                client.retrieve_file_start(path, cmd_offset, &()).await?;
            retrieve_file(client, data_stream, tokio::io::stdout()).await
        }
    }
}
