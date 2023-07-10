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

use std::env;
use std::path::PathBuf;
use std::str::FromStr;

use anyhow::{anyhow, Context};
use clap::{value_parser, Arg, ArgAction, Command, ValueHint};
use log::info;
use once_cell::sync::OnceCell;

use g3_compat::CpuAffinity;
use g3_daemon::opts::DaemonArgs;

const ARGS_VERSION: &str = "version";
const ARGS_VERBOSE: &str = "verbose";
const ARGS_DAEMON: &str = "daemon";
const ARGS_SYSTEMD: &str = "systemd";
const ARGS_GROUP_NAME: &str = "group-name";
const ARGS_CONFIG_FILE: &str = "config-file";
const ARGS_CONTROL_DIR: &str = "control-dir";
const ARGS_PID_FILE: &str = "pid-file";

static DAEMON_GROUP: OnceCell<String> = OnceCell::new();

pub struct ProcArgs {
    pub daemon_config: DaemonArgs,
    pub core_affinity: Option<CpuAffinity>,
    pub openssl_async_job: Option<usize>,
}

impl Default for ProcArgs {
    fn default() -> Self {
        ProcArgs {
            daemon_config: DaemonArgs::new(crate::build::PKG_NAME),
            core_affinity: None,
            openssl_async_job: None,
        }
    }
}

impl ProcArgs {
    fn check_openssl_async_job(&mut self) {
        if env::var("OPENSSL_CONF").is_ok() {
            let s = env::var("OPENSSL_ASYNC_JOB_SIZE").unwrap_or("1024".to_string());
            let async_job_size = usize::from_str(&s).unwrap_or(1024);
            self.openssl_async_job = Some(async_job_size);
        }
    }
}

fn build_cli_args() -> Command {
    Command::new(crate::build::PKG_NAME)
        .disable_version_flag(true)
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
                .default_value(g3_daemon::opts::DEFAULT_CONTROL_DIR)
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

    let mut proc_args = ProcArgs::default();

    if let Some(verbose_level) = args.get_one::<u8>(ARGS_VERBOSE) {
        proc_args.daemon_config.verbose_level = *verbose_level;
    }

    if args.get_flag(ARGS_VERSION) {
        crate::build::print_version(proc_args.daemon_config.verbose_level);
        return Ok(None);
    }
    if args.get_flag(ARGS_DAEMON) {
        proc_args.daemon_config.daemon_mode = true;
    }
    if args.get_flag(ARGS_SYSTEMD) {
        proc_args.daemon_config.set_with_systemd();
    }
    if let Some(config_file) = args.get_one::<PathBuf>(ARGS_CONFIG_FILE) {
        g3_daemon::opts::validate_and_set_config_file(config_file).context(format!(
            "failed to load config file {}",
            config_file.display()
        ))?;
    } else {
        return Err(anyhow!("no config file given"));
    }
    if let Some(control_dir) = args.get_one::<PathBuf>(ARGS_CONTROL_DIR) {
        g3_daemon::opts::validate_and_set_control_dir(control_dir)
            .context(format!("invalid control dir: {}", control_dir.display()))?;
    }
    if let Some(pid_file) = args.get_one::<PathBuf>(ARGS_PID_FILE) {
        proc_args.daemon_config.pid_file = Some(pid_file.to_path_buf());
    }
    if let Some(group_name) = args.get_one::<String>(ARGS_GROUP_NAME) {
        DAEMON_GROUP
            .set(group_name.to_string())
            .map_err(|_| anyhow!("daemon group has already been set"))?;

        if let Some(s) = group_name.strip_prefix("core") {
            let mut cpu = CpuAffinity::default();
            if let Ok(id) = usize::from_str(s) {
                if cpu.add_id(id).is_ok() {
                    info!("will try to bind to cpu core {id}");
                    proc_args.core_affinity = Some(cpu);
                }
            }
        }
    }

    proc_args.check_openssl_async_job();
    Ok(Some(proc_args))
}

pub(crate) fn daemon_group() -> &'static str {
    DAEMON_GROUP.get().map(|s| s.as_str()).unwrap_or_default()
}
