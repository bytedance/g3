/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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

    #[cfg(any(feature = "rustls-aws-lc", feature = "rustls-aws-lc-fips"))]
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .unwrap();
    #[cfg(feature = "rustls-ring")]
    rustls::crypto::ring::default_provider()
        .install_default()
        .unwrap();
    #[cfg(not(any(
        feature = "rustls-aws-lc",
        feature = "rustls-aws-lc-fips",
        feature = "rustls-ring"
    )))]
    compile_error!("either rustls-aws-lc or rustls-ring should be enabled");

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
