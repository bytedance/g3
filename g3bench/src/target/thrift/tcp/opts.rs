/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;
use std::time::Duration;

use anyhow::{Context, anyhow};
use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};
use tokio::net::TcpStream;

use g3_types::collection::{SelectiveVec, WeightedValue};
use g3_types::net::UpstreamAddr;

use crate::module::socket::{AppendSocketArgs, SocketArgs};
use crate::opts::ProcArgs;
use crate::target::thrift::{AppendThriftArgs, ThriftGlobalArgs};

const ARG_TARGET: &str = "target";
const ARG_CONNECT_TIMEOUT: &str = "connect-timeout";
const ARG_TIMEOUT: &str = "timeout";
const ARG_NO_KEEPALIVE: &str = "no-keepalive";

pub(super) struct ThriftTcpArgs {
    pub(super) global: ThriftGlobalArgs,
    target: UpstreamAddr,
    pub(super) timeout: Duration,
    pub(super) connect_timeout: Duration,
    pub(super) no_keepalive: bool,

    socket: SocketArgs,

    target_addrs: Option<SelectiveVec<WeightedValue<SocketAddr>>>,
}

impl ThriftTcpArgs {
    fn new(global_args: ThriftGlobalArgs, target: UpstreamAddr) -> Self {
        ThriftTcpArgs {
            global: global_args,
            target,
            timeout: Duration::from_secs(5),
            connect_timeout: Duration::from_secs(10),
            no_keepalive: false,
            socket: SocketArgs::default(),
            target_addrs: None,
        }
    }

    pub(super) async fn resolve_target_address(
        &mut self,
        proc_args: &ProcArgs,
    ) -> anyhow::Result<()> {
        let addrs = proc_args.resolve(&self.target).await?;
        self.target_addrs = Some(addrs);
        Ok(())
    }

    pub(super) async fn new_tcp_connection(
        &self,
        proc_args: &ProcArgs,
    ) -> anyhow::Result<TcpStream> {
        let addrs = self
            .target_addrs
            .as_ref()
            .ok_or_else(|| anyhow!("no target addr set"))?;
        let peer = *proc_args.select_peer(addrs);

        self.socket.tcp_connect_to(peer).await
    }
}

pub(super) fn add_tcp_args(app: Command) -> Command {
    app.arg(
        Arg::new(ARG_TARGET)
            .help("Target service address")
            .value_name("ADDRESS")
            .long(ARG_TARGET)
            .required(true)
            .num_args(1)
            .value_parser(value_parser!(UpstreamAddr)),
    )
    .arg(
        Arg::new(ARG_CONNECT_TIMEOUT)
            .value_name("TIMEOUT DURATION")
            .help("Timeout for connection to next peer")
            .default_value("10s")
            .long(ARG_CONNECT_TIMEOUT)
            .num_args(1),
    )
    .arg(
        Arg::new(ARG_TIMEOUT)
            .value_name("TIMEOUT DURATION")
            .help("Timeout for a single request")
            .default_value("5s")
            .long(ARG_TIMEOUT)
            .num_args(1),
    )
    .arg(
        Arg::new(ARG_NO_KEEPALIVE)
            .help("Disable keepalive")
            .action(ArgAction::SetTrue)
            .long(ARG_NO_KEEPALIVE),
    )
    .append_socket_args()
    .append_thrift_args()
}

pub(super) fn parse_tcp_args(args: &ArgMatches) -> anyhow::Result<ThriftTcpArgs> {
    let target = if let Some(v) = args.get_one::<UpstreamAddr>(ARG_TARGET) {
        v.clone()
    } else {
        return Err(anyhow!("no target set"));
    };

    let global_args =
        ThriftGlobalArgs::parse_args(args).context("failed to parse global thrift args")?;

    let mut t_args = ThriftTcpArgs::new(global_args, target);

    if let Some(timeout) = g3_clap::humanize::get_duration(args, ARG_CONNECT_TIMEOUT)? {
        t_args.connect_timeout = timeout;
    }
    if let Some(timeout) = g3_clap::humanize::get_duration(args, ARG_TIMEOUT)? {
        t_args.timeout = timeout;
    }

    if args.get_flag(ARG_NO_KEEPALIVE) {
        t_args.no_keepalive = true;
    }

    t_args
        .socket
        .parse_args(args)
        .context("invalid socket config")?;

    Ok(t_args)
}
