/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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
