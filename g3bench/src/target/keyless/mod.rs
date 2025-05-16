/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::process::ExitCode;
use std::sync::Arc;

use anyhow::anyhow;
use clap::{ArgMatches, Command};

use super::{BenchTarget, BenchTaskContext, ProcArgs};

mod opts;
use opts::{AppendKeylessArgs, KeylessGlobalArgs};

mod cloudflare;

mod openssl;

pub const COMMAND: &str = "keyless";

pub fn command() -> Command {
    Command::new(COMMAND)
        .subcommand_required(true)
        .subcommand_value_name("PROVIDER")
        .subcommand(openssl::command())
        .subcommand(cloudflare::command())
}

pub async fn run(proc_args: &Arc<ProcArgs>, cmd_args: &ArgMatches) -> anyhow::Result<ExitCode> {
    match cmd_args.subcommand() {
        Some((openssl::COMMAND, args)) => openssl::run(proc_args, args).await,
        Some((cloudflare::COMMAND, args)) => cloudflare::run(proc_args, args).await,
        Some((provider, _)) => Err(anyhow!("invalid provider {provider}")),
        None => Err(anyhow!("no provider set")),
    }
}
