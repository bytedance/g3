/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::anyhow;
use clap::ArgMatches;
use openssl::hash::DigestBytes;
use openssl::pkey::PKey;

use g3_ctl::CommandResult;
use g3_tls_cert::ext::PublicKeyExt;

pub const COMMAND_CHECK_DUP: &str = "check-dup";

const COMMAND_ARG_DIR: &str = "dir";
const COMMAND_ARG_EXT: &str = "ext";

pub mod commands {
    use super::*;
    use clap::{Arg, Command, ValueHint, value_parser};

    pub fn check_dup() -> Command {
        Command::new(COMMAND_CHECK_DUP)
            .arg(
                Arg::new(COMMAND_ARG_DIR)
                    .help("Directory that contains Private key files")
                    .required(true)
                    .num_args(1)
                    .value_parser(value_parser!(PathBuf))
                    .value_hint(ValueHint::FilePath),
            )
            .arg(
                Arg::new(COMMAND_ARG_EXT)
                    .help("File extension to match")
                    .long(COMMAND_ARG_EXT)
                    .num_args(1),
            )
    }
}

pub fn check_dup(args: &ArgMatches) -> CommandResult<()> {
    let dir_path = args.get_one::<PathBuf>(COMMAND_ARG_DIR).unwrap();
    let ext = args.get_one::<String>(COMMAND_ARG_EXT);

    let mut map = HashMap::new();

    let dir = fs::read_dir(dir_path)
        .map_err(|e| anyhow!("failed to open {}: {e}", dir_path.display()))?;
    for entry in dir {
        let entry = entry
            .map_err(|e| anyhow!("failed to read entry of dir {}: {e}", dir_path.display()))?;
        let path = entry.path();
        let ft = entry
            .file_type()
            .map_err(|e| anyhow!("failed to get file type of {}: {e}", path.display()))?;
        if !ft.is_file() {
            continue;
        }

        if let Some(ext) = ext {
            let Some(e) = path.extension() else {
                continue;
            };
            let Some(s) = e.to_str() else {
                continue;
            };
            if s != ext {
                continue;
            }
        }

        match get_ski(&path) {
            Ok(ski) => {
                if let Some(existed) = map.insert(ski.to_vec(), path.clone()) {
                    println!(
                        "dup SKI {}: {} - {} ",
                        hex::encode(ski),
                        existed.display(),
                        path.display()
                    );
                }
            }
            Err(e) => {
                eprintln!("{e}");
            }
        }
    }
    Ok(())
}

fn get_ski(path: &Path) -> anyhow::Result<DigestBytes> {
    let content = fs::read_to_string(path)
        .map_err(|e| anyhow!("failed to read content of file {}: {e}", path.display()))?;
    let key = PKey::private_key_from_pem(content.as_bytes())
        .map_err(|e| anyhow!("invalid private key pem file {}: {e}", path.display()))?;
    key.ski()
        .map_err(|e| anyhow!("failed to get SKI for key file {}: {e}", path.display()))
}
