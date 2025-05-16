/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;

use anyhow::anyhow;
use clap::{Arg, Command, value_parser};
use clap_complete::Shell;
use mlua::Lua;

mod cmd_path;
mod cmd_run;
mod cmd_version;

const GLOBAL_ARG_COMPLETION: &str = "completion";

fn build_cli_args() -> Command {
    Command::new("g3proxy-lua")
        .arg(
            Arg::new(GLOBAL_ARG_COMPLETION)
                .num_args(1)
                .value_name("SHELL")
                .long("completion")
                .value_parser(value_parser!(Shell))
                .exclusive(true),
        )
        .subcommand_required(true)
        .subcommand(cmd_version::command())
        .subcommand(cmd_path::command())
        .subcommand(cmd_run::command())
}

fn main() -> anyhow::Result<()> {
    let args = build_cli_args().get_matches();

    if let Some(target) = args.get_one::<Shell>(GLOBAL_ARG_COMPLETION) {
        let mut app = build_cli_args();
        let bin_name = app.get_name().to_string();
        clap_complete::generate(*target, &mut app, bin_name, &mut io::stdout());
        return Ok(());
    }

    let lua = unsafe { Lua::unsafe_new() };

    if let Some((cmd, args)) = args.subcommand() {
        match cmd {
            cmd_version::COMMAND => cmd_version::display(&lua),
            cmd_path::COMMAND => cmd_path::display(&lua),
            cmd_run::COMMAND => cmd_run::run(&lua, args),
            _ => Err(anyhow!("invalid subcommand {cmd}")),
        }
    } else {
        Ok(())
    }
}
