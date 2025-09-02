/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use anyhow::{Context, anyhow};
use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};

use crate::module::http::{AppendH3ConnectArgs, AppendHttpArgs, H3ConnectArgs, HttpClientArgs};

const HTTP_ARG_CONNECTION_POOL: &str = "connection-pool";
const HTTP_ARG_NO_MULTIPLEX: &str = "no-multiplex";

pub(super) struct BenchH3Args {
    pub(super) common: HttpClientArgs,
    pub(super) connect: H3ConnectArgs,
    pub(super) pool_size: Option<usize>,
    pub(super) no_multiplex: bool,
}

impl BenchH3Args {
    fn new(common: HttpClientArgs) -> Self {
        BenchH3Args {
            common,
            connect: H3ConnectArgs::default(),
            pool_size: None,
            no_multiplex: false,
        }
    }
}

pub(super) fn add_h3_args(app: Command) -> Command {
    app.arg(
        Arg::new(HTTP_ARG_CONNECTION_POOL)
            .help(
                "Set the number of pooled underlying h3 connections.\n\
                        If not set, each concurrency will use it's own h3 connection",
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
            .help("Disable h3 connection multiplexing")
            .action(ArgAction::SetTrue)
            .long(HTTP_ARG_NO_MULTIPLEX)
            .conflicts_with(HTTP_ARG_CONNECTION_POOL),
    )
    .append_http_args()
    .append_h3_connect_args()
}

pub(super) fn parse_h3_args(args: &ArgMatches) -> anyhow::Result<BenchH3Args> {
    let common = HttpClientArgs::parse_args(args)?;
    let mut h3_args = BenchH3Args::new(common);

    if let Some(c) = args.get_one::<usize>(HTTP_ARG_CONNECTION_POOL)
        && *c > 0
    {
        h3_args.pool_size = Some(*c);
    }

    if args.get_flag(HTTP_ARG_NO_MULTIPLEX) {
        h3_args.no_multiplex = true;
    }

    h3_args
        .connect
        .parse_args(args)
        .context("invalid h3 connect args")?;

    if h3_args.common.target_url.scheme() != "https" {
        return Err(anyhow!(
            "unsupported target url {}",
            h3_args.common.target_url
        ));
    }

    Ok(h3_args)
}
