/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::process::ExitCode;
use std::sync::Arc;

use anyhow::anyhow;
use clap::{ArgMatches, Command};

use crate::ProcArgs;

mod opts;
use opts::{AppendWebsocketArgs, WebsocketArgs};

mod frame;
use frame::{ClientFrameBuilder, FrameType, ServerFrameHeader};

mod stats;
use stats::{WebsocketHistogram, WebsocketHistogramRecorder};

mod h1;

pub const COMMAND: &str = "websocket";

pub fn command() -> Command {
    Command::new(COMMAND)
        .subcommand_required(true)
        .subcommand_value_name("HTTP VERSION")
        .subcommand(h1::command())
}

pub async fn run(proc_args: &Arc<ProcArgs>, cmd_args: &ArgMatches) -> anyhow::Result<ExitCode> {
    match cmd_args.subcommand() {
        Some((h1::COMMAND, args)) => h1::run(proc_args, args).await,
        Some((version, _)) => Err(anyhow!("invalid http version {version}")),
        None => Err(anyhow!("no http version set")),
    }
}
