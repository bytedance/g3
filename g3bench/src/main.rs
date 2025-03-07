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

use std::io;
use std::process::ExitCode;
use std::sync::Arc;

use anyhow::{Context, anyhow};
use clap::{Arg, ArgMatches, Command, value_parser};
use clap_complete::Shell;

const COMMAND_VERSION: &str = "version";
const COMMAND_COMPLETION: &str = "completion";

fn build_cli_args() -> Command {
    g3bench::add_global_args(Command::new(g3bench::build::PKG_NAME))
        .subcommand_required(true)
        .subcommand_value_name("TARGET")
        .subcommand(Command::new(COMMAND_VERSION).override_help("Show version"))
        .subcommand(
            Command::new(COMMAND_COMPLETION).arg(
                Arg::new("target")
                    .value_name("SHELL")
                    .required(true)
                    .num_args(1)
                    .value_parser(value_parser!(Shell)),
            ),
        )
        .subcommand(g3bench::target::h1::command())
        .subcommand(g3bench::target::h2::command())
        .subcommand(g3bench::target::h3::command())
        .subcommand(g3bench::target::openssl::command())
        .subcommand(g3bench::target::rustls::command())
        .subcommand(g3bench::target::dns::command())
        .subcommand(g3bench::target::keyless::command())
}

fn main() -> anyhow::Result<ExitCode> {
    #[cfg(feature = "openssl-probe")]
    unsafe {
        openssl_probe::init_openssl_env_vars();
    }
    openssl::init();

    #[cfg(feature = "rustls-ring")]
    rustls::crypto::ring::default_provider()
        .install_default()
        .unwrap();

    let args = build_cli_args().get_matches();
    let proc_args = g3bench::parse_global_args(&args)?;
    let proc_args = Arc::new(proc_args);

    let (subcommand, sub_args) = args
        .subcommand()
        .ok_or_else(|| anyhow!("no subcommand found"))?;

    match subcommand {
        COMMAND_VERSION => {
            g3bench::build::print_version();
            return Ok(ExitCode::SUCCESS);
        }
        COMMAND_COMPLETION => {
            generate_completion(sub_args);
            return Ok(ExitCode::SUCCESS);
        }
        _ => {}
    }

    proc_args.summary();

    let _worker_guard = if let Some(worker_config) = proc_args.worker_runtime() {
        let guard =
            g3bench::worker::spawn_workers(worker_config).context("failed to start workers")?;
        Some(guard)
    } else {
        None
    };

    let rt = proc_args
        .main_runtime()
        .start()
        .context("failed to start main runtime")?;
    rt.block_on(async move {
        match subcommand {
            g3bench::target::h1::COMMAND => g3bench::target::h1::run(&proc_args, sub_args).await,
            g3bench::target::h2::COMMAND => g3bench::target::h2::run(&proc_args, sub_args).await,
            g3bench::target::h3::COMMAND => g3bench::target::h3::run(&proc_args, sub_args).await,
            g3bench::target::openssl::COMMAND => {
                g3bench::target::openssl::run(&proc_args, sub_args).await
            }
            g3bench::target::rustls::COMMAND => {
                g3bench::target::rustls::run(&proc_args, sub_args).await
            }
            g3bench::target::dns::COMMAND => g3bench::target::dns::run(&proc_args, sub_args).await,
            g3bench::target::keyless::COMMAND => {
                g3bench::target::keyless::run(&proc_args, sub_args).await
            }
            cmd => Err(anyhow!("invalid subcommand {}", cmd)),
        }
    })
}

fn generate_completion(args: &ArgMatches) {
    if let Some(target) = args.get_one::<Shell>("target") {
        let mut app = build_cli_args();
        let bin_name = app.get_name().to_string();
        clap_complete::generate(*target, &mut app, bin_name, &mut io::stdout());
    }
}
