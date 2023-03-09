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

#[link(name = "g3-compat", kind = "static", modifiers = "+whole-archive")]
extern "C" {
    // ...
}

use anyhow::Context;
use log::{debug, error, info};

use g3tiles::opts::ProcArgs;

fn main() -> anyhow::Result<()> {
    openssl::init();

    let Some(proc_args) = g3tiles::opts::parse_clap().context("failed to parse command line options")? else {
        return Ok(());
    };

    // set up process logger early, only proc args is used inside
    let _log_guard = g3_daemon::log::process::setup(&proc_args.daemon_config)
        .context("failed to setup logger")?;

    g3tiles::config::load(&proc_args)
        .context(format!("failed to load config, opts: {:?}", &proc_args))?;
    debug!("loaded config from {}", proc_args.config_file.display());

    if proc_args.test_config {
        info!("the format of the config file is ok");
        return Ok(());
    }

    // enter daemon mode after config loaded
    g3_daemon::daemonize::check_enter(&proc_args.daemon_config)?;

    let stat_join = if let Some(stat_config) = g3_daemon::stat::config::get_global_stat_config() {
        Some(
            g3tiles::stat::spawn_working_threads(stat_config)
                .context("failed to start stat thread")?,
        )
    } else {
        None
    };

    let ret = tokio_run(&proc_args);

    if let Some(handlers) = stat_join {
        g3tiles::stat::stop_working_threads();
        for handle in handlers {
            let _ = handle.join();
        }
    }

    match ret {
        Ok(_) => Ok(()),
        Err(e) => {
            error!("{:?}", e);
            Err(e)
        }
    }
}

fn tokio_run(args: &ProcArgs) -> anyhow::Result<()> {
    let rt = g3_daemon::runtime::config::get_runtime_config()
        .start()
        .context("failed to start runtime")?;
    rt.block_on(async {
        let ret: anyhow::Result<()> = Ok(());

        g3_daemon::control::bridge::set_main_runtime_handle();
        let ctl_thread_handler = g3tiles::control::capnp::spawn_working_thread().await?;

        let unique_ctl = g3tiles::control::UniqueController::start()
            .context("failed to start unique controller")?;

        if args.daemon_config.daemon_mode || args.daemon_config.with_systemd {
            let daemon_ctl = g3tiles::control::DaemonController::start()
                .context("failed to start daemon controller")?;
            tokio::spawn(async move {
                daemon_ctl.await;
            });
        }

        g3tiles::signal::setup_and_spawn().context("failed to setup signal handler")?;

        // TODO

        let _workers_guard = g3_daemon::runtime::worker::spawn_workers()
            .await
            .context("failed to spawn workers")?;
        g3tiles::serve::spawn_offline_clean();
        g3tiles::serve::spawn_all()
            .await
            .context("failed to spawn all servers")?;

        unique_ctl.await;

        g3tiles::control::capnp::stop_working_thread();
        let _ = ctl_thread_handler.join();

        ret
    })
}
