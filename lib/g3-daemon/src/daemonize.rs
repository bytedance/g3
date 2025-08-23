/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::fs::OpenOptions;

use anyhow::anyhow;
use daemonize::Daemonize;
use log::debug;

use crate::opts::DaemonArgs;

pub fn check_enter(args: &DaemonArgs) -> anyhow::Result<()> {
    if args.daemon_mode {
        enter(args)?;
        debug!("enter daemon mode");
    }
    Ok(())
}

fn enter(args: &DaemonArgs) -> anyhow::Result<()> {
    let stdout = OpenOptions::new().write(true).open("/dev/null")?;
    let stderr = OpenOptions::new().write(true).open("/dev/null")?;
    let mut daemonize = Daemonize::new()
        .working_directory("/tmp")
        .stdout(stdout)
        .stderr(stderr);

    if let Some(pid_file) = &args.pid_file {
        daemonize = daemonize.pid_file(pid_file).chown_pid_file(true);
    }

    daemonize.start().map_err(|e| anyhow!("daemonize: {e}"))
}
