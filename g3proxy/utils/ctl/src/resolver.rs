/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use anyhow::anyhow;
use clap::{Arg, ArgMatches, Command, value_parser};
use futures_util::future::TryFutureExt;

use g3_ctl::{CommandError, CommandResult};
use g3_types::resolve::QueryStrategy as ResolverQueryStrategy;

use g3proxy_proto::proc_capnp::proc_control;
use g3proxy_proto::resolver_capnp::{
    QueryStrategy as RpcQueryStrategy, query_result, resolver_control,
};

pub const COMMAND: &str = "resolver";

const COMMAND_ARG_NAME: &str = "name";

const SUBCOMMAND_QUERY: &str = "query";
const SUBCOMMAND_QUERY_ARG_DOMAIN: &str = "domain";
const SUBCOMMAND_QUERY_ARG_STRATEGY: &str = "strategy";
const SUBCOMMAND_QUERY_ARG_RESOLUTION_DELAY: &str = "resolution-delay";

pub fn command() -> Command {
    Command::new(COMMAND)
        .arg(Arg::new(COMMAND_ARG_NAME).required(true).num_args(1))
        .subcommand_required(true)
        .subcommand(
            Command::new(SUBCOMMAND_QUERY)
                .arg(Arg::new(SUBCOMMAND_QUERY_ARG_DOMAIN).required(true))
                .arg(
                    Arg::new(SUBCOMMAND_QUERY_ARG_STRATEGY)
                        .short('s')
                        .long("strategy")
                        .alias("query-strategy")
                        .num_args(1),
                )
                .arg(
                    Arg::new(SUBCOMMAND_QUERY_ARG_RESOLUTION_DELAY)
                        .long(SUBCOMMAND_QUERY_ARG_RESOLUTION_DELAY)
                        .num_args(1)
                        .value_parser(value_parser!(u16))
                        .default_value("50"),
                ),
        )
}

async fn query_domain(client: &resolver_control::Client, args: &ArgMatches) -> CommandResult<()> {
    let domain = args.get_one::<String>(SUBCOMMAND_QUERY_ARG_DOMAIN).unwrap();
    let mut req = client.query_request();
    req.get().set_domain(domain);

    if let Some(delay) = args.get_one::<u16>(SUBCOMMAND_QUERY_ARG_RESOLUTION_DELAY) {
        req.get().set_resolution_delay(*delay);
    }

    if let Some(qs) = args.get_one::<String>(SUBCOMMAND_QUERY_ARG_STRATEGY) {
        let qs = ResolverQueryStrategy::from_str(qs)
            .map_err(|_| CommandError::Cli(anyhow!("invalid query strategy")))?;
        let qs = match qs {
            ResolverQueryStrategy::Ipv4Only => RpcQueryStrategy::Ipv4Only,
            ResolverQueryStrategy::Ipv4First => RpcQueryStrategy::Ipv4First,
            ResolverQueryStrategy::Ipv6Only => RpcQueryStrategy::Ipv6Only,
            ResolverQueryStrategy::Ipv6First => RpcQueryStrategy::Ipv6First,
        };
        req.get().set_strategy(qs);
    }

    let rsp = req.send().promise.await?;
    let result = rsp.get()?.get_result()?;
    match result.which().unwrap() {
        query_result::Which::Ip(ips) => {
            let ips = ips?;
            g3_ctl::print_text_list("ip", ips)
        }
        query_result::Which::Err(reason) => g3_ctl::print_text("err", reason?),
    }
}

pub async fn run(client: &proc_control::Client, args: &ArgMatches) -> CommandResult<()> {
    let name = args.get_one::<String>(COMMAND_ARG_NAME).unwrap();

    let (subcommand, args) = args.subcommand().unwrap();
    match subcommand {
        SUBCOMMAND_QUERY => {
            super::proc::get_resolver(client, name)
                .and_then(|resolver| async move { query_domain(&resolver, args).await })
                .await
        }
        _ => unreachable!(),
    }
}
