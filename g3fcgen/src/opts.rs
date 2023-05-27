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

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context};
use clap::{value_parser, Arg, ArgAction, ArgMatches, Command, ValueHint};

use g3_daemon::opts::DaemonArgs;
use g3_runtime::blended::BlendedRuntimeConfig;

const GLOBAL_ARG_VERBOSE: &str = "verbose";
const GLOBAL_ARG_VERSION: &str = "version";
const GLOBAL_ARG_DAEMON: &str = "daemon";
const GLOBAL_ARG_SYSTEMD: &str = "systemd";
const GLOBAL_ARG_PID_FILE: &str = "pid-file";
const GLOBAL_ARG_CA_CERT: &str = "ca-cert";
const GLOBAL_ARG_CA_KEY: &str = "ca-key";
const GLOBAL_ARG_UDP_ADDR: &str = "udp";

const GLOBAL_ARG_BACKEND_NUMBER: &str = "backend-number";
const GLOBAL_ARG_THREAD_NUMBER: &str = "thread-number";

pub struct ProcArgs {
    pub daemon_config: DaemonArgs,
    pub(crate) ca_cert: String,
    pub(crate) ca_key: String,
    pub(crate) udp_addr: Option<SocketAddr>,
    pub print_version: bool,
    pub runtime_config: BlendedRuntimeConfig,
    pub(crate) backend_number: usize,
}

impl Default for ProcArgs {
    fn default() -> Self {
        ProcArgs {
            daemon_config: DaemonArgs::new(crate::build::PKG_NAME),
            ca_cert: String::new(),
            ca_key: String::new(),
            udp_addr: None,
            print_version: false,
            runtime_config: BlendedRuntimeConfig::default(),
            backend_number: 1,
        }
    }
}

pub fn add_global_args(app: Command) -> Command {
    app.arg(
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
        Arg::new(GLOBAL_ARG_CA_CERT)
            .help("ca certificate file, in PEM format")
            .value_name("CA CERTIFICATE FILE")
            .long(GLOBAL_ARG_CA_CERT)
            .required_unless_present_any([GLOBAL_ARG_VERSION])
            .num_args(1)
            .value_parser(value_parser!(PathBuf))
            .value_hint(ValueHint::FilePath),
    )
    .arg(
        Arg::new(GLOBAL_ARG_CA_KEY)
            .help("ca private key file, in PEM format")
            .value_name("CA PRIVATE KEY FILE")
            .long(GLOBAL_ARG_CA_KEY)
            .required_unless_present_any([GLOBAL_ARG_VERSION])
            .num_args(1)
            .value_parser(value_parser!(PathBuf))
            .value_hint(ValueHint::FilePath),
    )
    .arg(
        Arg::new(GLOBAL_ARG_UDP_ADDR)
            .help("the udp socket address to accept requests")
            .value_name("UDP SOCKET ADDRESS")
            .long(GLOBAL_ARG_UDP_ADDR)
            .short('u')
            .num_args(1)
            .value_parser(value_parser!(SocketAddr))
            .default_value("127.0.0.1:2999"),
    )
    .arg(
        Arg::new(GLOBAL_ARG_THREAD_NUMBER)
            .help("runtime thread number")
            .value_name("THREAD COUNT")
            .long(GLOBAL_ARG_THREAD_NUMBER)
            .short('T')
            .num_args(1)
            .value_parser(value_parser!(usize)),
    )
    .arg(
        Arg::new(GLOBAL_ARG_BACKEND_NUMBER)
            .help("number of background helper process")
            .value_name("BACKEND COUNT")
            .long(GLOBAL_ARG_BACKEND_NUMBER)
            .short('N')
            .num_args(1)
            .value_parser(value_parser!(usize))
            .default_value("1"),
    )
}

pub fn parse_global_args(args: &ArgMatches) -> anyhow::Result<ProcArgs> {
    let mut proc_args = ProcArgs::default();

    if let Some(verbose_level) = args.get_one::<u8>(GLOBAL_ARG_VERBOSE) {
        proc_args.daemon_config.verbose_level = *verbose_level;
    }

    if args.get_flag(GLOBAL_ARG_VERSION) {
        proc_args.print_version = true;
        return Ok(proc_args);
    }

    if args.get_flag(GLOBAL_ARG_DAEMON) {
        proc_args.daemon_config.daemon_mode = true;
    }
    if args.get_flag(GLOBAL_ARG_SYSTEMD) {
        proc_args.daemon_config.set_with_systemd();
    }

    if let Some(pid_file) = args.get_one::<PathBuf>(GLOBAL_ARG_PID_FILE) {
        proc_args.daemon_config.pid_file = Some(pid_file.to_path_buf());
    }

    if let Some(thread_number) = args.get_one::<usize>(GLOBAL_ARG_THREAD_NUMBER) {
        proc_args.runtime_config.set_thread_number(*thread_number);
    }

    if let Some(backend_number) = args.get_one::<usize>(GLOBAL_ARG_BACKEND_NUMBER) {
        proc_args.backend_number = *backend_number;
    }

    let ca_cert_file = args.get_one::<PathBuf>(GLOBAL_ARG_CA_CERT).unwrap();
    proc_args.ca_cert = load_ca_cert(ca_cert_file).context(format!(
        "failed to load ca cert file {}",
        ca_cert_file.display()
    ))?;

    let ca_key_file = args.get_one::<PathBuf>(GLOBAL_ARG_CA_KEY).unwrap();
    proc_args.ca_key = load_ca_key(ca_key_file).context(format!(
        "failed to load ca key file {}",
        ca_key_file.display()
    ))?;

    proc_args.udp_addr = args.get_one::<SocketAddr>(GLOBAL_ARG_UDP_ADDR).cloned();

    Ok(proc_args)
}

fn load_ca_cert(path: &Path) -> anyhow::Result<String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| anyhow!("failed to read in file: {e:?}"))?;
    Ok(content)
}

fn load_ca_key(path: &Path) -> anyhow::Result<String> {
    let content =
        std::fs::read_to_string(path).map_err(|e| anyhow!("failed to read in file: {e:?}"))?;
    Ok(content)
}
