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
use std::path::PathBuf;

use clap::{value_parser, Arg, ArgAction, ArgMatches, Command, ValueHint};
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

    let verbose_level = args
        .get_one::<u8>(COMMAND_ARG_VERBOSE)
        .copied()
        .unwrap_or_default();

    let code = std::fs::read_to_string(script)
        .map_err(|e| anyhow!("failed to read script file {}: {e:?}", script.display()))?;

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
