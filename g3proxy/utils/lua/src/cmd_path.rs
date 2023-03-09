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
use clap::Command;
use mlua::Lua;

pub const COMMAND: &str = "path";

pub fn command() -> Command {
    Command::new(COMMAND)
}

pub fn display(lua: &Lua) -> anyhow::Result<()> {
    let path = lua
        .load("package.path")
        .eval::<String>()
        .map_err(|e| anyhow!("failed to load lua path: {e}"))?;

    let cpath = lua
        .load("package.cpath")
        .eval::<String>()
        .map_err(|e| anyhow!("failed to load lua cpath: {e}"))?;

    println!("lua path: {path}");
    println!("lua cpath: {cpath}");

    Ok(())
}
