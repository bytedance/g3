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
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::OnceLock;

use anyhow::{anyhow, Context};
use clap::{value_parser, Arg, ArgAction, Command, ValueHint};

use g3_daemon::opts::{DaemonArgs, DaemonArgsExt};
use g3_types::net::{SocketBufferConfig, UdpListenConfig};

const GLOBAL_ARG_VERSION: &str = "version";
const GLOBAL_ARG_GROUP_NAME: &str = "group-name";
const GLOBAL_ARG_CONFIG_FILE: &str = "config-file";
const GLOBAL_ARG_RECV_BUF_SIZE: &str = "recv-buf-size";
const GLOBAL_ARG_SEND_BUF_SIZE: &str = "send-buf-size";

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
            listen: UdpListenConfig::new(SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 2999)),
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
                .long(GLOBAL_ARG_GROUP_NAME),
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
                .long(GLOBAL_ARG_CONFIG_FILE),
        )
        .arg(
            Arg::new(GLOBAL_ARG_RECV_BUF_SIZE)
                .help("Set receive buffer size for the listen socket")
                .num_args(1)
                .value_name("BUF SIZE")
                .value_parser(value_parser!(usize))
                .long(GLOBAL_ARG_RECV_BUF_SIZE),
        )
        .arg(
            Arg::new(GLOBAL_ARG_SEND_BUF_SIZE)
                .help("Set send buffer size for the listen socket")
                .num_args(1)
                .value_name("BUF SIZE")
                .value_parser(value_parser!(usize))
                .long(GLOBAL_ARG_SEND_BUF_SIZE),
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

    let mut socket_buffer_config = SocketBufferConfig::default();
    if let Some(size) = args.get_one::<usize>(GLOBAL_ARG_RECV_BUF_SIZE) {
        socket_buffer_config.set_recv_size(*size);
    }
    if let Some(size) = args.get_one::<usize>(GLOBAL_ARG_SEND_BUF_SIZE) {
        socket_buffer_config.set_send_size(*size);
    }
    proc_args.listen.set_socket_buffer(socket_buffer_config);

    Ok(Some(proc_args))
}
