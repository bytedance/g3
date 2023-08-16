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

use std::net::{IpAddr, Ipv6Addr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::OnceLock;

use anyhow::{anyhow, Context};
use clap::{value_parser, Arg, ArgAction, Command, ValueHint};

use g3_daemon::opts::DaemonArgs;

const GLOBAL_ARG_VERBOSE: &str = "verbose";
const GLOBAL_ARG_VERSION: &str = "version";
const GLOBAL_ARG_TEST_CONFIG: &str = "test-config";
const GLOBAL_ARG_DAEMON: &str = "daemon";
const GLOBAL_ARG_SYSTEMD: &str = "systemd";
const GLOBAL_ARG_GROUP_NAME: &str = "group-name";
const GLOBAL_ARG_PID_FILE: &str = "pid-file";
const GLOBAL_ARG_CONFIG_FILE: &str = "config-file";

static DAEMON_GROUP: OnceLock<String> = OnceLock::new();

#[derive(Debug)]
pub struct ProcArgs {
    pub daemon_config: DaemonArgs,
    pub test_config: bool,
    pub(crate) udp_addr: Option<SocketAddr>,
}

impl Default for ProcArgs {
    fn default() -> Self {
        ProcArgs {
            daemon_config: DaemonArgs::new(crate::build::PKG_NAME),
            test_config: false,
            udp_addr: None,
        }
    }
}

fn build_cli_args() -> Command {
    Command::new(crate::build::PKG_NAME)
        .disable_version_flag(true)
        .arg(
            Arg::new(GLOBAL_ARG_VERBOSE)
                .help("Show verbose output")
                .num_args(0)
                .action(ArgAction::Count)
                .short('v')
                .long(GLOBAL_ARG_VERBOSE),
        )
        .arg(
            Arg::new(GLOBAL_ARG_VERSION)
                .help("Show version")
                .num_args(0)
                .action(ArgAction::SetTrue)
                .short('V')
                .long(GLOBAL_ARG_VERSION),
        )
        .arg(
            Arg::new(GLOBAL_ARG_TEST_CONFIG)
                .help("Test the format of config file and exit")
                .action(ArgAction::SetTrue)
                .short('t')
                .long("test-config"),
        )
        .arg(
            Arg::new(GLOBAL_ARG_DAEMON)
                .help("Run in daemon mode")
                .action(ArgAction::SetTrue)
                .requires_all([GLOBAL_ARG_PID_FILE])
                .short('d')
                .long(GLOBAL_ARG_DAEMON),
        )
        .arg(
            Arg::new(GLOBAL_ARG_SYSTEMD)
                .help("Run with systemd")
                .action(ArgAction::SetTrue)
                .short('s')
                .long(GLOBAL_ARG_SYSTEMD),
        )
        .arg(
            Arg::new(GLOBAL_ARG_GROUP_NAME)
                .help("Group name")
                .num_args(1)
                .value_name("GROUP NAME")
                .short('G')
                .long("group-name"),
        )
        .arg(
            Arg::new(GLOBAL_ARG_PID_FILE)
                .help("Pid file for daemon mode")
                .num_args(1)
                .value_name("PID FILE")
                .value_hint(ValueHint::FilePath)
                .value_parser(value_parser!(PathBuf))
                .short('p')
                .long(GLOBAL_ARG_PID_FILE),
        )
        .arg(
            Arg::new(GLOBAL_ARG_CONFIG_FILE)
                .help("Config file path")
                .num_args(1)
                .value_name("CONFIG FILE")
                .value_hint(ValueHint::FilePath)
                .value_parser(value_parser!(PathBuf))
                .required_unless_present_any([GLOBAL_ARG_TEST_CONFIG])
                .short('c')
                .long("config-file"),
        )
}

pub fn parse_clap() -> anyhow::Result<Option<ProcArgs>> {
    let args_parser = build_cli_args();
    let args = args_parser.get_matches();

    let mut proc_args = ProcArgs::default();

    if let Some(verbose_level) = args.get_one::<u8>(GLOBAL_ARG_VERBOSE) {
        proc_args.daemon_config.verbose_level = *verbose_level;
    }

    if args.get_flag(GLOBAL_ARG_VERSION) {
        crate::build::print_version(proc_args.daemon_config.verbose_level);
        return Ok(None);
    }
    if args.get_flag(GLOBAL_ARG_TEST_CONFIG) {
        proc_args.test_config = true;
    }
    if args.get_flag(GLOBAL_ARG_DAEMON) {
        proc_args.daemon_config.daemon_mode = true;
    }
    if args.get_flag(GLOBAL_ARG_SYSTEMD) {
        proc_args.daemon_config.set_with_systemd();
    }

    if let Some(config_file) = args.get_one::<PathBuf>(GLOBAL_ARG_CONFIG_FILE) {
        g3_daemon::opts::validate_and_set_config_file(config_file).context(format!(
            "failed to load config file {}",
            config_file.display()
        ))?;
    } else {
        return Err(anyhow!("no config file given"));
    }
    if let Some(pid_file) = args.get_one::<PathBuf>(GLOBAL_ARG_PID_FILE) {
        proc_args.daemon_config.pid_file = Some(pid_file.to_path_buf());
    }

    if let Some(group_name) = args.get_one::<String>(GLOBAL_ARG_GROUP_NAME) {
        DAEMON_GROUP
            .set(group_name.to_string())
            .map_err(|_| anyhow!("daemon group has already been set"))?;

        if let Some(s) = group_name.strip_prefix("port") {
            if let Ok(port) = u16::from_str(s) {
                proc_args.udp_addr = Some(SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), port));
            }
        }
    }

    if let Some(s) = option_env!("UDP_LISTEN_ADDR") {
        if let Ok(addr) = SocketAddr::from_str(s) {
            proc_args.udp_addr = Some(addr);
        }
    }

    Ok(Some(proc_args))
}
