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
use clap::Command;

fn build_cli_args() -> Command {
    g3fcgen::add_global_args(Command::new(g3fcgen::build::PKG_NAME))
}

fn main() -> anyhow::Result<()> {
    #[cfg(feature = "vendored-openssl")]
    openssl_probe::init_ssl_cert_env_vars();
    openssl::init();

    let args = build_cli_args().get_matches();
    let proc_args = g3fcgen::parse_global_args(&args)?;

    if proc_args.print_version {
        g3fcgen::build::print_version(proc_args.daemon_config.verbose_level);
        return Ok(());
    }

    // set up process logger early, only proc args is used inside
    let _log_guard = g3_daemon::log::process::setup(&proc_args.daemon_config)
        .context("failed to setup logger")?;

    // enter daemon mode after config loaded
    g3_daemon::daemonize::check_enter(&proc_args.daemon_config)?;

    let rt = proc_args
        .runtime_config
        .start()
        .context("failed to start runtime")?;
    rt.block_on(g3fcgen::run(&proc_args))
}
