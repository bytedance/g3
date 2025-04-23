/*
 * Copyright 2025 ByteDance and/or its affiliates.
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
