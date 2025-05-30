/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::env;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::OnceLock;

use anyhow::{Context, anyhow};
use clap::{Arg, ArgAction, Command, ValueHint, value_parser};

use g3_daemon::opts::{DaemonArgs, DaemonArgsExt};
use g3_types::net::UdpListenConfig;

const GLOBAL_ARG_VERSION: &str = "version";
const GLOBAL_ARG_GROUP_NAME: &str = "group-name";
const GLOBAL_ARG_CONFIG_FILE: &str = "config-file";

static DAEMON_GROUP: OnceLock<String> = OnceLock::new();

#[derive(Debug)]
pub struct ProcArgs {
    pub daemon_config: DaemonArgs,
    listen: UdpListenConfig,
}

impl Default for ProcArgs {
    fn default() -> Self {
        ProcArgs {
            daemon_config: DaemonArgs::new(crate::build::PKG_NAME),
            listen: UdpListenConfig::new(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 2888)),
        }
    }
}

impl ProcArgs {
    pub(crate) fn listen_config(&self) -> &UdpListenConfig {
        &self.listen
    }
}

fn build_cli_args() -> Command {
    Command::new(crate::build::PKG_NAME)
        .disable_version_flag(true)
        .append_daemon_args()
        .arg(
            Arg::new(GLOBAL_ARG_VERSION)
                .help("Show version")
                .num_args(0)
                .action(ArgAction::SetTrue)
                .short('V')
                .long(GLOBAL_ARG_VERSION),
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
            Arg::new(GLOBAL_ARG_CONFIG_FILE)
                .help("Config file path")
                .num_args(1)
                .value_name("CONFIG FILE")
                .value_hint(ValueHint::FilePath)
                .value_parser(value_parser!(PathBuf))
                .required_unless_present_any([GLOBAL_ARG_VERSION])
                .short('c')
                .long("config-file"),
        )
}

pub fn parse_clap() -> anyhow::Result<Option<ProcArgs>> {
    let args_parser = build_cli_args();
    let args = args_parser.get_matches();

    let mut proc_args = ProcArgs::default();
    proc_args.daemon_config.parse_clap(&args)?;

    if args.get_flag(GLOBAL_ARG_VERSION) {
        crate::build::print_version(proc_args.daemon_config.verbose_level);
        return Ok(None);
    }

    if let Some(config_file) = args.get_one::<PathBuf>(GLOBAL_ARG_CONFIG_FILE) {
        g3_daemon::opts::validate_and_set_config_file(config_file, crate::build::PKG_NAME)
            .context(format!(
                "failed to load config file {}",
                config_file.display()
            ))?;
    } else {
        return Err(anyhow!("no config file given"));
    }

    if let Some(group_name) = args.get_one::<String>(GLOBAL_ARG_GROUP_NAME) {
        DAEMON_GROUP
            .set(group_name.to_string())
            .map_err(|_| anyhow!("daemon group has already been set"))?;

        if let Some(s) = group_name.strip_prefix("port") {
            if let Ok(port) = u16::from_str(s) {
                proc_args
                    .listen
                    .set_socket_address(SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), port));
            }
        }
    }

    if let Ok(s) = env::var("UDP_LISTEN_ADDR") {
        if let Ok(addr) = SocketAddr::from_str(&s) {
            proc_args.listen.set_socket_address(addr);
        }
    }

    Ok(Some(proc_args))
}
