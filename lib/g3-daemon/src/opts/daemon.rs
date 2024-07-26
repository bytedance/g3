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

use std::path::PathBuf;

use clap::{value_parser, Arg, ArgAction, ArgMatches, Command, ValueHint};

const ARGS_VERBOSE: &str = "verbose";
const ARGS_DAEMON: &str = "daemon";
const ARGS_SYSTEMD: &str = "systemd";
const ARGS_PID_FILE: &str = "pid-file";
const ARGS_TEST_CONFIG: &str = "test-config";

pub trait DaemonArgsExt {
    fn append_daemon_args(self) -> Self;
}

#[derive(Debug)]
pub struct DaemonArgs {
    pub(crate) with_systemd: bool,
    pub(crate) daemon_mode: bool,
    pub verbose_level: u8,
    pub process_name: &'static str,
    pub pid_file: Option<PathBuf>,
    pub test_config: bool,
}

impl DaemonArgs {
    pub fn new(process_name: &'static str) -> Self {
        DaemonArgs {
            with_systemd: false,
            daemon_mode: false,
            verbose_level: 0,
            process_name,
            pid_file: None,
            test_config: false,
        }
    }

    fn set_with_systemd(&mut self) {
        cfg_if::cfg_if! {
            if #[cfg(target_os = "linux")] {
                self.with_systemd = true;
            } else {
                self.with_systemd = false;
            }
        }
    }

    fn enable_daemon_mode(&mut self) {
        cfg_if::cfg_if! {
            if #[cfg(unix)] {
                self.daemon_mode = true;
            } else {
                self.daemon_mode = false;
            }
        }
    }

    pub fn need_daemon_controller(&self) -> bool {
        self.daemon_mode || self.with_systemd
    }

    pub fn parse_clap(&mut self, args: &ArgMatches) -> anyhow::Result<()> {
        if let Some(verbose_level) = args.get_one::<u8>(ARGS_VERBOSE) {
            self.verbose_level = *verbose_level;
        }
        if args.get_flag(ARGS_TEST_CONFIG) {
            self.test_config = true;
        }
        if args.get_flag(ARGS_DAEMON) {
            self.enable_daemon_mode();
        }
        if args.get_flag(ARGS_SYSTEMD) {
            self.set_with_systemd();
        }
        if let Some(pid_file) = args.get_one::<PathBuf>(ARGS_PID_FILE) {
            self.pid_file = Some(pid_file.to_path_buf());
        }
        Ok(())
    }
}

impl DaemonArgsExt for Command {
    fn append_daemon_args(self) -> Self {
        self.arg(
            Arg::new(ARGS_VERBOSE)
                .help("Show verbose output")
                .num_args(0)
                .action(ArgAction::Count)
                .short('v')
                .long("verbose"),
        )
        .arg(
            Arg::new(ARGS_DAEMON)
                .help("Run in daemon mode")
                .action(ArgAction::SetTrue)
                .requires(ARGS_PID_FILE)
                .short('d')
                .long("daemon"),
        )
        .arg(
            Arg::new(ARGS_SYSTEMD)
                .help("Run with systemd")
                .action(ArgAction::SetTrue)
                .short('s')
                .long("systemd"),
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
        .arg(
            Arg::new(ARGS_TEST_CONFIG)
                .help("Test the format of config file and exit")
                .action(ArgAction::SetTrue)
                .short('t')
                .long("test-config"),
        )
    }
}
