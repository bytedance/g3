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

use anyhow::Context;
use log::{debug, error, info};

use g3_daemon::control::{QuitAction, UpgradeAction};

use g3tiles::opts::ProcArgs;

fn main() -> anyhow::Result<()> {
    #[cfg(feature = "openssl-probe")]
    unsafe {
        openssl_probe::init_openssl_env_vars();
    }
    openssl::init();

    #[cfg(feature = "rustls-ring")]
    rustls::crypto::ring::default_provider()
        .install_default()
        .unwrap();

    let Some(proc_args) =
        g3tiles::opts::parse_clap().context("failed to parse command line options")?
    else {
        return Ok(());
    };

    // set up process logger early, only proc args is used inside
    g3_daemon::log::process::setup(&proc_args.daemon_config);
    if proc_args.daemon_config.need_daemon_controller() {
        g3tiles::control::UpgradeActor::connect_to_old_daemon();
    }

    let config_file = match g3tiles::config::load() {
        Ok(c) => c,
        Err(e) => {
            g3_daemon::control::upgrade::cancel_old_shutdown();
            return Err(e.context(format!("failed to load config, opts: {:?}", &proc_args)));
        }
    };
    debug!("loaded config from {}", config_file.display());

    if proc_args.daemon_config.test_config {
        info!("the format of the config file is ok");
        return Ok(());
    }

    // enter daemon mode after config loaded
    #[cfg(unix)]
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

        g3_daemon::runtime::set_main_handle();

        let ctl_thread_handler = g3tiles::control::capnp::spawn_working_thread().await?;

        let unique_ctl = g3tiles::control::UniqueController::start()
            .context("failed to start unique controller")?;
        if args.daemon_config.need_daemon_controller() {
            g3_daemon::control::upgrade::release_old_controller().await;
            let daemon_ctl = g3tiles::control::DaemonController::start()
                .context("failed to start daemon controller")?;
            tokio::spawn(async move {
                daemon_ctl.await;
            });
        }
        g3tiles::control::QuitActor::tokio_spawn_run();

        g3tiles::signal::register().context("failed to setup signal handler")?;

        let _workers_guard = g3_daemon::runtime::worker::spawn_workers()
            .await
            .context("failed to spawn workers")?;
        match load_and_spawn().await {
            Ok(_) => g3_daemon::control::upgrade::finish(),
            Err(e) => {
                g3_daemon::control::upgrade::cancel_old_shutdown();
                return Err(e);
            }
        }

        unique_ctl.await;

        g3tiles::control::capnp::stop_working_thread();
        let _ = ctl_thread_handler.join();

        ret
    })
}

async fn load_and_spawn() -> anyhow::Result<()> {
    g3tiles::discover::load_all()
        .await
        .context("failed to load all discovers")?;
    g3tiles::backend::load_all()
        .await
        .context("failed to load all connectors")?;
    g3tiles::serve::spawn_offline_clean();
    g3tiles::serve::spawn_all()
        .await
        .context("failed to spawn all servers")?;
    Ok(())
}
