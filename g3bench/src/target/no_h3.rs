/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::process::ExitCode;
use std::sync::Arc;

use anyhow::anyhow;
use clap::{ArgMatches, Command};

use crate::ProcArgs;

pub const COMMAND: &str = "h3";

pub fn command() -> Command {
    Command::new(COMMAND).hide(true)
}

pub async fn run(_proc_args: &Arc<ProcArgs>, _cmd_args: &ArgMatches) -> anyhow::Result<ExitCode> {
    Err(anyhow!(
        "h3 support is not compiled in, 'quic' feature is needed to enable this"
    ))
}
