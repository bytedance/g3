/*
 * Copyright 2025 ByteDance and/or its affiliates.
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

use anyhow::Context;
use log::{debug, error, info};

use g3statsd::opts::ProcArgs;

fn main() -> anyhow::Result<()> {
    let Some(proc_args) =
        g3statsd::opts::parse_clap().context("failed to parse command line options")?
    else {
        return Ok(());
    };

    // set up process logger early, only proc args is used inside
    g3_daemon::log::process::setup(&proc_args.daemon_config);

    let config_file = g3statsd::config::load()
        .context(format!("failed to load config, opts: {:?}", &proc_args))?;
    debug!("loaded config from {}", config_file.display());

    if proc_args.daemon_config.test_config {
        info!("the format of the config file is ok");
        return Ok(());
    }

    // enter daemon mode after config loaded
    #[cfg(unix)]
    g3_daemon::daemonize::check_enter(&proc_args.daemon_config)?;

    let ret = tokio_run(&proc_args);

    match ret {
        Ok(_) => Ok(()),
        Err(e) => {
            error!("{:?}", e);
            Err(e)
        }
    }
}

fn tokio_run(_args: &ProcArgs) -> anyhow::Result<()> {
    let rt = g3_daemon::runtime::config::get_runtime_config()
        .start()
        .context("failed to start runtime")?;
    rt.block_on(async {
        g3_daemon::runtime::set_main_handle();

        // TODO setup output
        g3statsd::input::spawn_all()
            .await
            .context("failed to spawn all inputs")?;

        Ok(())
    })
}
