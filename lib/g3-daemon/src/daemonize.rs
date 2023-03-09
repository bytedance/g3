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

use std::fs::OpenOptions;

use anyhow::anyhow;
use daemonize::Daemonize;
use log::debug;

use crate::DaemonArgs;

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
