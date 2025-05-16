/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::path::PathBuf;
use std::sync::OnceLock;

use anyhow::{Context, anyhow};
use clap::builder::ArgPredicate;
use clap::{Arg, ArgAction, Command, ValueHint, value_parser};
use clap_complete::Shell;

use g3_daemon::opts::{DaemonArgs, DaemonArgsExt};

const ARGS_COMPLETION: &str = "completion";
const ARGS_VERSION: &str = "version";
const ARGS_GROUP_NAME: &str = "group-name";
const ARGS_CONFIG_FILE: &str = "config-file";
const ARGS_CONTROL_DIR: &str = "control-dir";

static DAEMON_GROUP: OnceLock<String> = OnceLock::new();

#[derive(Debug)]
pub struct ProcArgs {
    pub daemon_config: DaemonArgs,
}

impl Default for ProcArgs {
    fn default() -> Self {
        ProcArgs {
            daemon_config: DaemonArgs::new(crate::build::PKG_NAME),
        }
    }
}

fn build_cli_args() -> Command {
    Command::new(crate::build::PKG_NAME)
        .disable_version_flag(true)
        .append_daemon_args()
        .arg(
            Arg::new(ARGS_COMPLETION)
                .num_args(1)
                .value_name("SHELL")
                .long("completion")
                .value_parser(value_parser!(Shell))
                .exclusive(true),
        )
        .arg(
            Arg::new(ARGS_VERSION)
                .help("Show version")
                .num_args(0)
                .action(ArgAction::SetTrue)
                .short('V')
                .long(ARGS_VERSION),
        )
        .arg(
            Arg::new(ARGS_GROUP_NAME)
                .help("Group name")
                .num_args(1)
                .value_name("GROUP NAME")
                .short('G')
                .long("group-name"),
        )
        .arg(
            Arg::new(ARGS_CONTROL_DIR)
                .help("Control socket directory")
                .num_args(1)
                .value_name("CONTROL DIR")
                .value_hint(ValueHint::DirPath)
                .value_parser(value_parser!(PathBuf))
                .default_value(g3_daemon::opts::DEFAULT_CONTROL_DIR)
                .default_value_if(ARGS_COMPLETION, ArgPredicate::IsPresent, None)
                .short('C')
                .long("control-dir"),
        )
        .arg(
            Arg::new(ARGS_CONFIG_FILE)
                .help("Config file path")
                .num_args(1)
                .value_name("CONFIG FILE")
                .value_hint(ValueHint::FilePath)
                .value_parser(value_parser!(PathBuf))
                .required_unless_present_any([ARGS_VERSION])
                .short('c')
                .long(ARGS_CONFIG_FILE),
        )
}

pub fn parse_clap() -> anyhow::Result<Option<ProcArgs>> {
    let args_parser = build_cli_args();
    let args = args_parser.get_matches();

    if let Some(target) = args.get_one::<Shell>(ARGS_COMPLETION) {
        let mut app = build_cli_args();
        let bin_name = app.get_name().to_string();
        clap_complete::generate(*target, &mut app, bin_name, &mut io::stdout());
        return Ok(None);
    }

    let mut proc_args = ProcArgs::default();
    proc_args.daemon_config.parse_clap(&args)?;

    if args.get_flag(ARGS_VERSION) {
        crate::build::print_version(proc_args.daemon_config.verbose_level);
        return Ok(None);
    }

    if let Some(config_file) = args.get_one::<PathBuf>(ARGS_CONFIG_FILE) {
        g3_daemon::opts::validate_and_set_config_file(config_file, crate::build::PKG_NAME)
            .context(format!(
                "failed to load config file {}",
                config_file.display()
            ))?;
    } else {
        return Err(anyhow!("no config file given"));
    }
    #[cfg(unix)]
    if let Some(control_dir) = args.get_one::<PathBuf>(ARGS_CONTROL_DIR) {
        g3_daemon::opts::validate_and_set_control_dir(control_dir)
            .context(format!("invalid control dir: {}", control_dir.display()))?;
    }
    if let Some(group_name) = args.get_one::<String>(ARGS_GROUP_NAME) {
        DAEMON_GROUP
            .set(group_name.to_string())
            .map_err(|_| anyhow!("daemon group has already been set"))?;
    }

    Ok(Some(proc_args))
}

pub(crate) fn daemon_group() -> &'static str {
    DAEMON_GROUP.get().map(|s| s.as_str()).unwrap_or_default()
}
