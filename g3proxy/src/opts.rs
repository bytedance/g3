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
use std::path::PathBuf;
use std::sync::OnceLock;

use anyhow::{anyhow, Context};
use clap::builder::ArgPredicate;
use clap::{value_parser, Arg, ArgAction, Command, ValueHint};
use clap_complete::Shell;

use g3_daemon::opts::{DaemonArgs, DaemonArgsExt};

const ARGS_COMPLETION: &str = "completion";
const ARGS_VERSION: &str = "version";
const ARGS_VERIFY_PANIC: &str = "verify-panic";
const ARGS_DEP_GRAPH: &str = "dep-graph";
const ARGS_GROUP_NAME: &str = "group-name";
const ARGS_CONFIG_FILE: &str = "config-file";
const ARGS_CONTROL_DIR: &str = "control-dir";

const DEP_GRAPH_GRAPHVIZ: &str = "graphviz";
const DEP_GRAPH_MERMAID: &str = "mermaid";
const DEP_GRAPH_PLANTUML: &str = "plantuml";

static DAEMON_GROUP: OnceLock<String> = OnceLock::new();

#[derive(Debug)]
pub struct ProcArgs {
    pub daemon_config: DaemonArgs,
    pub output_graphviz_graph: bool,
    pub output_mermaid_graph: bool,
    pub output_plantuml_graph: bool,
}

impl Default for ProcArgs {
    fn default() -> Self {
        ProcArgs {
            daemon_config: DaemonArgs::new(crate::build::PKG_NAME),
            output_graphviz_graph: false,
            output_mermaid_graph: false,
            output_plantuml_graph: false,
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
            Arg::new(ARGS_DEP_GRAPH)
                .help("Generate a dependency graph")
                .value_name("GRAPH TYPE")
                .short('g')
                .long("dep-graph")
                .num_args(0..=1)
                .value_parser([DEP_GRAPH_GRAPHVIZ, DEP_GRAPH_MERMAID, DEP_GRAPH_PLANTUML])
                .default_missing_value(DEP_GRAPH_GRAPHVIZ),
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
                .required_unless_present_any([ARGS_COMPLETION, ARGS_VERSION, ARGS_VERIFY_PANIC])
                .short('c')
                .long("config-file"),
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
    if args.get_flag(ARGS_VERIFY_PANIC) {
        panic!("panic as requested")
    }
    if let Some(g) = args.get_one::<String>(ARGS_DEP_GRAPH) {
        match g.as_str() {
            DEP_GRAPH_GRAPHVIZ => proc_args.output_graphviz_graph = true,
            DEP_GRAPH_MERMAID => proc_args.output_mermaid_graph = true,
            DEP_GRAPH_PLANTUML => proc_args.output_plantuml_graph = true,
            s => {
                panic!("unsupported graph type {s}")
            }
        }
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
