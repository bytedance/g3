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

use anyhow::{anyhow, Context};
use log::{debug, error, info, warn};

use g3keymess::opts::ProcArgs;

fn main() -> anyhow::Result<()> {
    #[cfg(feature = "vendored-openssl")]
    openssl_probe::init_ssl_cert_env_vars();
    openssl::init();

    let Some(proc_args) = g3keymess::opts::parse_clap().context("failed to parse command line options")? else {
        return Ok(());
    };

    if let Some(cpu_affinity) = &proc_args.core_affinity {
        if let Err(e) = cpu_affinity.apply_to_local_thread() {
            warn!("failed to apply cpu affinity: {e}");
        }
    }

    // set up process logger early, only proc args is used inside
    let _log_guard = g3_daemon::log::process::setup(&proc_args.daemon_config)
        .context("failed to setup logger")?;

    let config_file = g3keymess::config::load().context("failed to load config")?;
    debug!("loaded config from {}", config_file.display());

    // enter daemon mode after config loaded
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

fn tokio_run(args: &ProcArgs) -> anyhow::Result<()> {
    let mut rt_builder = tokio::runtime::Builder::new_current_thread();
    rt_builder.enable_all();

    if let Some(async_job_size) = args.openssl_async_job {
        info!("will init {async_job_size} openssl async jobs");
        rt_builder.on_thread_start(move || {
            if let Err(e) = openssl_async_job::async_thread_init(async_job_size, async_job_size) {
                warn!("failed to init {async_job_size} openssl async jobs: {e}");
            }
        });
        rt_builder.on_thread_stop(openssl_async_job::async_thread_cleanup);
    }

    let rt = rt_builder
        .build()
        .map_err(|e| anyhow!("failed to start runtime: {e}"))?;
    rt.block_on(async {
        let ret: anyhow::Result<()> = Ok(());

        g3_daemon::control::bridge::set_main_runtime_handle();
        // TODO capnp

        let unique_ctl = g3keymess::control::UniqueController::start()
            .context("failed to start unique controller")?;

        if args.daemon_config.need_daemon_controller() {
            let daemon_ctl = g3keymess::control::DaemonController::start()
                .context("failed to start daemon controller")?;
            tokio::spawn(async move {
                daemon_ctl.await;
            });
        }

        g3keymess::signal::setup_and_spawn().context("failed to setup signal handler")?;

        g3keymess::store::load_all()
            .await
            .context("failed to load all key stores")?;

        g3keymess::serve::spawn_offline_clean();
        g3keymess::serve::spawn_all()
            .await
            .context("failed to spawn all servers")?;

        unique_ctl.await;

        ret
    })
}
