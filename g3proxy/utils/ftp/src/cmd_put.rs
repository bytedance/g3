/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::path::PathBuf;

use anyhow::{Context, anyhow};
use clap::{Arg, ArgMatches, Command, ValueHint, value_parser};
use tokio::fs::File;
use tokio::io::{AsyncRead, AsyncWrite};

use g3_ftp_client::{FtpClient, FtpConnectionProvider};

pub(super) const COMMAND: &str = "put";

const COMMAND_ARG_PATH: &str = "path";
const COMMAND_ARG_FILE: &str = "file";

pub(super) fn command() -> Command {
    Command::new(COMMAND)
        .about("Upload file")
        .arg(
            Arg::new(COMMAND_ARG_PATH)
                .help("remote upload path")
                .value_name("FILE PATH")
                .num_args(1)
                .required(true),
        )
        .arg(
            Arg::new(COMMAND_ARG_FILE)
                .help("local file to upload")
                .value_name("FILE")
                .num_args(1)
                .value_hint(ValueHint::FilePath)
                .value_parser(value_parser!(PathBuf))
                .long("file")
                .required(true),
        )
}

async fn store_file<CP, S, E, F>(
    ftp_client: &mut FtpClient<CP, S, E, ()>,
    mut data_stream: S,
    mut file_stream: F,
) -> anyhow::Result<()>
where
    CP: FtpConnectionProvider<S, E, ()>,
    S: AsyncRead + AsyncWrite + Unpin,
    E: std::error::Error,
    F: AsyncRead + Unpin,
{
    let data_copy = tokio::io::copy(&mut file_stream, &mut data_stream);

    tokio::select! {
        biased;

        r = data_copy => {
            drop(data_stream);
            ftp_client
                .wait_store_end_reply()
                .await
                .context("failed to recv store end reply")?;
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
            ftp_client.wait_store_end_reply()
                .await
                .context("failed to recv store end reply")?;

            Err(anyhow!("server send end reply code before we close transfer stream"))
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
    let file = args.get_one::<PathBuf>(COMMAND_ARG_FILE).unwrap();

    let file_stream = File::open(file)
        .await
        .context(format!("failed to open local file {}", file.display()))?;
    let data_stream = client.store_file_start(path, &()).await?;
    store_file(client, data_stream, file_stream).await
}
