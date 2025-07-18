/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::process::ExitCode;
use std::sync::Arc;

use anyhow::anyhow;
use clap::{ArgMatches, Command};

use crate::ProcArgs;

mod tcp;

pub const COMMAND: &str = "thrift";

pub fn command() -> Command {
    Command::new(COMMAND)
        .subcommand_required(true)
        .subcommand_value_name("TRANSPORT")
        .subcommand(tcp::command())
}

pub async fn run(proc_args: &Arc<ProcArgs>, cmd_args: &ArgMatches) -> anyhow::Result<ExitCode> {
    match cmd_args.subcommand() {
        Some((tcp::COMMAND, args)) => tcp::run(proc_args, args).await,
        Some((transport, _)) => Err(anyhow!("invalid transport {transport}")),
        None => Err(anyhow!("no transport set")),
    }
}
