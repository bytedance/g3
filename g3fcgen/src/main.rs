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

use g3fcgen::opts::ProcArgs;

fn main() -> anyhow::Result<()> {
    #[cfg(feature = "openssl-probe")]
    openssl_probe::init_ssl_cert_env_vars();
    openssl::init();

    let Some(proc_args) =
        g3fcgen::opts::parse_clap().context("failed to parse command line options")?
    else {
        return Ok(());
    };

    // set up process logger early, only proc args is used inside
    let _log_guard = g3_daemon::log::process::setup(&proc_args.daemon_config)
        .context("failed to setup logger")?;

    g3_daemon::runtime::config::set_default_thread_number(0); // default to use current thread
    let config_file = g3fcgen::config::load()
        .context(format!("failed to load config, opts: {:?}", &proc_args))?;
    debug!("loaded config from {}", config_file.display());

    if proc_args.daemon_config.test_config {
        info!("the format of the config file is ok");
        return Ok(());
    }

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
    let rt = g3_daemon::runtime::config::get_runtime_config()
        .start()
        .context("failed to start runtime")?;
    rt.block_on(async {
        // TODO setup signal handler

        let _workers_guard = g3_daemon::runtime::worker::spawn_workers()
            .await
            .context("failed to spawn workers")?;

        g3fcgen::run(args).await
    })
}
