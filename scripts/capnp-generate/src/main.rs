/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::path::PathBuf;

use anyhow::anyhow;
use clap::{Arg, Command, ValueHint, value_parser};

const ARG_DIR: &str = "dir";

fn main() -> anyhow::Result<()> {
    let args = Command::new(env!("CARGO_PKG_NAME"))
        .arg(
            Arg::new(ARG_DIR)
                .required(true)
                .num_args(1)
                .value_parser(value_parser!(PathBuf))
                .value_hint(ValueHint::DirPath),
        )
        .get_matches();

    let dir = args.get_one::<PathBuf>(ARG_DIR).unwrap();
    let schema_dir = dir.join("schema");

    let mut capnp_command = capnpc::CompilerCommand::new();
    capnp_command.src_prefix(&schema_dir);

    let d = std::fs::read_dir(&schema_dir)
        .map_err(|e| anyhow!("failed to open schema dir {}: {e}", schema_dir.display()))?;
    for e in d {
        let Ok(e) = e else { continue };

        capnp_command.file(e.path());
    }

    let generate_dir = dir.join("gen");
    capnp_command
        .output_path(generate_dir)
        .run()
        .map_err(|e| anyhow!("failed to generate: {e}"))
}
