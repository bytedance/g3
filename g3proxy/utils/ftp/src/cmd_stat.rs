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
