/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::fs::OpenOptions;
use std::io::Write;

use anyhow::anyhow;
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
    #[allow(deprecated)] // daemon() deprecated on MacOS but still usable
    unsafe {
        let r = libc::daemon(0, 0);
        if r != 0 {
            let e = std::io::Error::last_os_error();
            return Err(anyhow!("daemon(0, 0) failed: {e}"));
        }
    }

    if let Some(pid_file) = &args.pid_file {
        let mut pidfile = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(false)
            .open(pid_file)
            .map_err(|e| anyhow!("failed to open pid file {}: {e}", pid_file.display()))?;
        pidfile
            .lock()
            .map_err(|e| anyhow!("failed to lock pid file {}: {e}", pid_file.display()))?;
        pidfile
            .set_len(0)
            .map_err(|e| anyhow!("failed to truncate pid file {}: {e}", pid_file.display()))?;
        let pid = rustix::process::getpid();
        pidfile.write_fmt(format_args!("{pid}\n"))?;
    }

    Ok(())
}
