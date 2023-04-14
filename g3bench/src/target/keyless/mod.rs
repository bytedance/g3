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

use anyhow::anyhow;
use std::sync::Arc;

use clap::{ArgMatches, Command};

use super::{BenchTarget, BenchTaskContext, ProcArgs};

mod cloudflare;

pub const COMMAND: &str = "keyless";

pub fn command() -> Command {
    Command::new(COMMAND)
        .display_name("PROVIDER")
        .subcommand(cloudflare::command())
}

pub async fn run(proc_args: &Arc<ProcArgs>, cmd_args: &ArgMatches) -> anyhow::Result<()> {
    match cmd_args.subcommand() {
        Some((cloudflare::COMMAND, args)) => cloudflare::run(proc_args, args).await,
        Some((provider, _)) => Err(anyhow!("invalid provider {provider}")),
        None => Err(anyhow!("no provider set")),
    }
}
