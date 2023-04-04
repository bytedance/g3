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

use std::io;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context};
use clap::builder::ArgPredicate;
use clap::{value_parser, Arg, ArgAction, Command, ValueHint};
use clap_complete::Shell;
use once_cell::sync::OnceCell;

use g3_daemon::DaemonArgs;

const DEFAULT_CONTROL_DIR: &str = "/tmp/g3";

const ARGS_COMPLETION: &str = "completion";
const ARGS_VERSION: &str = "version";
const ARGS_VERBOSE: &str = "verbose";
const ARGS_VERIFY_PANIC: &str = "verify-panic";
const ARGS_TEST_CONFIG: &str = "test-config";
const ARGS_DEP_GRAPH: &str = "dep-graph";
const ARGS_DAEMON: &str = "daemon";
const ARGS_SYSTEMD: &str = "systemd";
const ARGS_GROUP_NAME: &str = "group-name";
const ARGS_CONFIG_FILE: &str = "config-file";
const ARGS_CONTROL_DIR: &str = "control-dir";
const ARGS_PID_FILE: &str = "pid-file";

const DEP_GRAPH_GRAPHVIZ: &str = "graphviz";
const DEP_GRAPH_MERMAID: &str = "mermaid";

static CONTROL_DIR: OnceCell<PathBuf> = OnceCell::new();

#[derive(Debug)]
pub struct ProcArgs {
    pub daemon_config: DaemonArgs,
    pub config_file: PathBuf,
    pub group_name: String,
    pub test_config: bool,
    pub output_dot_graph: bool,
    pub output_mermaid_graph: bool,
}

impl Default for ProcArgs {
    fn default() -> Self {
        ProcArgs::new(PathBuf::new())
    }
}

impl ProcArgs {
    pub fn new(config_file: PathBuf) -> Self {
        ProcArgs {
            daemon_config: DaemonArgs::new(crate::build::PKG_NAME),
            config_file,
            group_name: String::new(),
            test_config: false,
            output_dot_graph: false,
            output_mermaid_graph: false,
        }
    }
}

fn build_cli_args() -> Command {
    Command::new(crate::build::PKG_NAME)
        .disable_version_flag(true)
        .arg(
            Arg::new(ARGS_COMPLETION)
                .num_args(1)
                .value_name("SHELL")
                .long("completion")
                .value_parser(value_parser!(Shell))
                .exclusive(true),
        )
        .arg(
            Arg::new(ARGS_VERBOSE)
                .help("Show verbose output")
                .num_args(0)
                .action(ArgAction::Count)
                .short('v')
                .long("verbose"),
        )
        .arg(
            Arg::new(ARGS_VERSION)
                .help("Show version")
                .action(ArgAction::SetTrue)
                .short('V')
                .long("version"),
        )
        .arg(
            Arg::new(ARGS_VERIFY_PANIC)
                .help("Verify panic message")
                .action(ArgAction::SetTrue)
                .hide(true)
                .long("verify-panic"),
        )
        .arg(
            Arg::new(ARGS_TEST_CONFIG)
                .help("Test the format of config file and exit")
                .action(ArgAction::SetTrue)
                .short('t')
                .long("test-config"),
        )
        .arg(
            Arg::new(ARGS_DEP_GRAPH)
                .help("Generate a dependency graph")
                .value_name("GRAPH TYPE")
                .short('g')
                .long("dep-graph")
                .num_args(0..=1)
                .value_parser([DEP_GRAPH_GRAPHVIZ, DEP_GRAPH_MERMAID])
                .default_missing_value(DEP_GRAPH_GRAPHVIZ),
        )
        .arg(
            Arg::new(ARGS_DAEMON)
                .help("Run in daemon mode")
                .action(ArgAction::SetTrue)
                .requires_all([ARGS_GROUP_NAME, ARGS_PID_FILE])
                .short('d')
                .long("daemon"),
        )
        .arg(
            Arg::new(ARGS_SYSTEMD)
                .help("Run with systemd")
                .action(ArgAction::SetTrue)
                .requires_all([ARGS_GROUP_NAME])
                .short('s')
                .long("systemd"),
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
                .default_value(DEFAULT_CONTROL_DIR)
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
                .required_unless_present_any([ARGS_COMPLETION, ARGS_VERSION, ARGS_VERIFY_PANIC])
                .short('c')
                .long("config-file"),
        )
        .arg(
            Arg::new(ARGS_PID_FILE)
                .help("Pid file for daemon mode")
                .num_args(1)
                .value_name("PID FILE")
                .value_hint(ValueHint::FilePath)
                .value_parser(value_parser!(PathBuf))
                .short('p')
                .long("pid-file"),
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

    if let Some(verbose_level) = args.get_one::<u8>(ARGS_VERBOSE) {
        proc_args.daemon_config.verbose_level = *verbose_level;
    }

    if args.get_flag(ARGS_VERSION) {
        crate::build::print_version(proc_args.daemon_config.verbose_level);
        return Ok(None);
    }
    if args.get_flag(ARGS_VERIFY_PANIC) {
        panic!("panic as requested")
    }
    if args.get_flag(ARGS_TEST_CONFIG) {
        proc_args.test_config = true;
    }
    if let Some(g) = args.get_one::<String>(ARGS_DEP_GRAPH) {
        match g.as_str() {
            DEP_GRAPH_GRAPHVIZ => proc_args.output_dot_graph = true,
            DEP_GRAPH_MERMAID => proc_args.output_mermaid_graph = true,
            s => {
                panic!("unsupported graph type {s}")
            }
        }
    }
    if args.get_flag(ARGS_DAEMON) {
        proc_args.daemon_config.daemon_mode = true;
    }
    if args.get_flag(ARGS_SYSTEMD) {
        proc_args.daemon_config.with_systemd = true;
    }
    if let Some(config_file) = args.get_one::<PathBuf>(ARGS_CONFIG_FILE) {
        proc_args.config_file = validate_and_get_config_file(config_file).context(format!(
            "failed to load config file {}",
            config_file.display()
        ))?;
    } else {
        return Err(anyhow!("no config file given"));
    }
    if let Some(control_dir) = args.get_one::<PathBuf>(ARGS_CONTROL_DIR) {
        validate_and_set_control_dir(control_dir)
            .context(format!("invalid control dir: {}", control_dir.display()))?;
    }
    if let Some(pid_file) = args.get_one::<PathBuf>(ARGS_PID_FILE) {
        proc_args.daemon_config.pid_file = Some(pid_file.to_path_buf());
    }
    if let Some(group_name) = args.get_one::<String>(ARGS_GROUP_NAME) {
        proc_args.group_name = group_name.to_string();
    }

    Ok(Some(proc_args))
}

fn validate_and_get_config_file(path: &Path) -> anyhow::Result<PathBuf> {
    if path.is_absolute() {
        return Ok(path.to_path_buf());
    }
    let mut dir = std::env::current_dir()?;
    dir.push(path);
    dir.canonicalize()?;
    Ok(dir)
}

fn validate_and_set_control_dir(path: &Path) -> anyhow::Result<()> {
    if path.is_relative() {
        return Err(anyhow!("{} is not an absolute path", path.display()));
    }

    if path.exists() && !path.is_dir() {
        return Err(anyhow!("{} is existed but not a directory", path.display()));
    }

    CONTROL_DIR
        .set(path.to_path_buf())
        .map_err(|_| anyhow!("control directory has already been set"))
}

pub(crate) fn control_dir() -> PathBuf {
    CONTROL_DIR
        .get()
        .cloned()
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CONTROL_DIR))
}
