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
use std::io;

use clap::{value_parser, Arg, Command};
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
