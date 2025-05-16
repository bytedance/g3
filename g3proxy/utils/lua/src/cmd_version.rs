/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use clap::Command;
use mlua::Lua;

pub const COMMAND: &str = "version";

pub fn command() -> Command {
    Command::new(COMMAND)
}

pub fn display(lua: &Lua) -> anyhow::Result<()> {
    let version = lua
        .globals()
        .get::<String>("_VERSION")
        .map_err(|e| anyhow!("failed to get _VERSION variable: {e}"))?;

    println!("lua version: {version}");
    Ok(())
}
