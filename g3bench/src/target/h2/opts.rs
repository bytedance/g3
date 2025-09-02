/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};

use crate::module::http::{AppendH2ConnectArgs, AppendHttpArgs, H2ConnectArgs, HttpClientArgs};

const HTTP_ARG_CONNECTION_POOL: &str = "connection-pool";
const HTTP_ARG_NO_MULTIPLEX: &str = "no-multiplex";

pub(super) struct BenchH2Args {
    pub(super) common: HttpClientArgs,
    pub(super) connect: H2ConnectArgs,
    pub(super) pool_size: Option<usize>,
    pub(super) no_multiplex: bool,
}

impl BenchH2Args {
    fn new(common: HttpClientArgs) -> Self {
        let connect = H2ConnectArgs::new(common.is_https());

        BenchH2Args {
            common,
            pool_size: None,
            connect,
            no_multiplex: false,
        }
    }
}

pub(super) fn add_h2_args(app: Command) -> Command {
    app.arg(
        Arg::new(HTTP_ARG_CONNECTION_POOL)
            .help(
                "Set the number of pooled underlying h2 connections.\n\
                        If not set, each concurrency will use it's own h2 connection",
            )
            .value_name("POOL SIZE")
            .long(HTTP_ARG_CONNECTION_POOL)
            .short('C')
            .num_args(1)
            .value_parser(value_parser!(usize))
            .conflicts_with(HTTP_ARG_NO_MULTIPLEX),
    )
    .arg(
        Arg::new(HTTP_ARG_NO_MULTIPLEX)
            .help("Disable h2 connection multiplexing")
            .action(ArgAction::SetTrue)
            .long(HTTP_ARG_NO_MULTIPLEX)
            .conflicts_with(HTTP_ARG_CONNECTION_POOL),
    )
    .append_http_args()
    .append_h2_connect_args()
}

pub(super) fn parse_h2_args(args: &ArgMatches) -> anyhow::Result<BenchH2Args> {
    let common = HttpClientArgs::parse_args(args)?;
    let mut h2_args = BenchH2Args::new(common);

    if let Some(c) = args.get_one::<usize>(HTTP_ARG_CONNECTION_POOL)
        && *c > 0
    {
        h2_args.pool_size = Some(*c);
    }

    if args.get_flag(HTTP_ARG_NO_MULTIPLEX) {
        h2_args.no_multiplex = true;
    }

    h2_args
        .connect
        .parse_args(args)
        .context("invalid h2 connect args")?;

    match h2_args.common.target_url.scheme() {
        "http" | "https" => {}
        _ => {
            return Err(anyhow!(
                "unsupported target url {}",
                h2_args.common.target_url
            ));
        }
    }

    Ok(h2_args)
}
