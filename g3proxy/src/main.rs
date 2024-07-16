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

use g3proxy::opts::ProcArgs;

fn main() -> anyhow::Result<()> {
    #[cfg(feature = "openssl-probe")]
    openssl_probe::init_ssl_cert_env_vars();
    openssl::init();

    #[cfg(feature = "rustls-aws-lc")]
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .unwrap();
    #[cfg(not(feature = "rustls-aws-lc"))]
    rustls::crypto::ring::default_provider()
        .install_default()
        .unwrap();

    let Some(proc_args) =
        g3proxy::opts::parse_clap().context("failed to parse command line options")?
    else {
        return Ok(());
    };

    // set up process logger early, only proc args is used inside
    let _log_guard = g3_daemon::log::process::setup(&proc_args.daemon_config);
    if proc_args.daemon_config.need_daemon_controller() {
        g3proxy::control::upgrade::connect_to_old_daemon();
    }

    let config_file = match g3proxy::config::load() {
        Ok(c) => c,
        Err(e) => {
            g3proxy::control::upgrade::cancel_old_shutdown();
            return Err(e.context(format!("failed to load config, opts: {:?}", &proc_args)));
        }
    };
    debug!("loaded config from {}", config_file.display());

    if proc_args.daemon_config.test_config {
        info!("the format of the config file is ok");
        return Ok(());
    }
    if proc_args.output_graphviz_graph {
        let content = g3proxy::config::graphviz_graph()?;
        println!("{content}");
        return Ok(());
    }
    if proc_args.output_mermaid_graph {
        let content = g3proxy::config::mermaid_graph()?;
        println!("{content}");
        return Ok(());
    }
    if proc_args.output_plantuml_graph {
        let content = g3proxy::config::plantuml_graph()?;
        println!("{content}");
        return Ok(());
    }

    // enter daemon mode after config loaded
    #[cfg(unix)]
    g3_daemon::daemonize::check_enter(&proc_args.daemon_config)?;

    let stat_join = if let Some(stat_config) = g3_daemon::stat::config::get_global_stat_config() {
        Some(
            g3proxy::stat::spawn_working_threads(stat_config)
                .context("failed to start stat thread")?,
        )
    } else {
        None
    };

    let ret = tokio_run(&proc_args);

    if let Some(handlers) = stat_join {
        g3proxy::stat::stop_working_threads();
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

        let ctl_thread_handler = g3proxy::control::capnp::spawn_working_thread().await?;

        let unique_ctl = g3proxy::control::UniqueController::start()
            .context("failed to start unique controller")?;
        if args.daemon_config.need_daemon_controller() {
            g3proxy::control::upgrade::release_old_controller().await;
            let daemon_ctl = g3proxy::control::DaemonController::start()
                .context("failed to start daemon controller")?;
            tokio::spawn(async move {
                daemon_ctl.await;
            });
        }
        g3proxy::control::QuitActor::spawn_run();

        g3proxy::signal::register().context("failed to setup signal handler")?;

        match load_and_spawn().await {
            Ok(_) => {}
            Err(e) => {
                g3proxy::control::upgrade::cancel_old_shutdown();
                return Err(e);
            }
        }

        unique_ctl.await;

        g3proxy::control::capnp::stop_working_thread();
        let _ = ctl_thread_handler.join();

        ret
    })?;

    Ok(())
}

async fn load_and_spawn() -> anyhow::Result<()> {
    g3proxy::resolve::spawn_all()
        .await
        .context("failed to spawn all resolvers")?;
    g3proxy::escape::load_all()
        .await
        .context("failed to load all escapers")?;
    g3proxy::auth::load_all()
        .await
        .context("failed to load all user groups")?;
    g3proxy::audit::load_all()
        .await
        .context("failed to load all auditors")?;
    let _workers_guard = g3_daemon::runtime::worker::spawn_workers()
        .await
        .context("failed to spawn workers")?;
    g3proxy::serve::spawn_offline_clean();
    g3proxy::serve::spawn_all()
        .await
        .context("failed to spawn all servers")?;
    Ok(())
}
