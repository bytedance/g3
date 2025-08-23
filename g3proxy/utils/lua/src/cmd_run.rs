/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::path::PathBuf;

use anyhow::anyhow;
use clap::{Arg, ArgAction, ArgMatches, Command, ValueHint, value_parser};
use mlua::{Lua, Value};

pub const COMMAND: &str = "run";

const COMMAND_ARG_SCRIPT: &str = "script";
const COMMAND_ARG_VERBOSE: &str = "verbose";

pub fn command() -> Command {
    Command::new(COMMAND)
        .arg(
            Arg::new(COMMAND_ARG_SCRIPT)
                .help("the script file to run")
                .value_name("SCRIPT FILE")
                .num_args(1)
                .value_parser(value_parser!(PathBuf))
                .value_hint(ValueHint::FilePath)
                .required(true),
        )
        .arg(
            Arg::new(COMMAND_ARG_VERBOSE)
                .help("output verbose level")
                .num_args(0)
                .action(ArgAction::Count)
                .short('v')
                .long("verbose"),
        )
}

pub fn run(lua: &Lua, args: &ArgMatches) -> anyhow::Result<()> {
    let script = args
        .get_one::<PathBuf>(COMMAND_ARG_SCRIPT)
        .ok_or_else(|| anyhow!("no script file to run"))?;
    let absolute_path = if !script.is_absolute() {
        let mut cur_dir = std::env::current_dir()?;
        cur_dir.push(script);
        cur_dir
    } else {
        script.to_path_buf()
    };

    let verbose_level = args
        .get_one::<u8>(COMMAND_ARG_VERBOSE)
        .copied()
        .unwrap_or_default();

    let code = std::fs::read_to_string(script)
        .map_err(|e| anyhow!("failed to read script file {}: {e:?}", script.display()))?;

    let globals = lua.globals();
    globals.set("__file__", absolute_path.display().to_string())?;
    let code = lua.load(&code);

    if verbose_level > 1 {
        println!("== script evaluation start ==");
    }
    let value = code
        .eval::<Value>()
        .map_err(|e| anyhow!("failed to run script file {}: {e}", script.display()))?;

    if verbose_level > 1 {
        println!("== script evaluation end ==");
        println!("== returned data <{}> start ==", value.type_name());
    }
    if verbose_level > 0 {
        match &value {
            Value::Nil => {}
            Value::String(s) => {
                println!("{}", s.to_string_lossy());
            }
            Value::Boolean(v) => {
                println!("{}", *v);
            }
            Value::Integer(i) => {
                println!("{}", *i);
            }
            Value::Number(n) => {
                println!("{}", *n);
            }
            Value::Error(e) => {
                println!("{e}");
            }
            _ => {}
        }
    }
    if verbose_level > 1 {
        println!("== returned data <{}> end ==", value.type_name());
    }

    Ok(())
}
