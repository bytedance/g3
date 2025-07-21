/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, anyhow};
use clap::{Arg, ArgAction, ArgGroup, ArgMatches, Command, value_parser};
use tokio::io::AsyncRead;
use tokio::net::TcpStream;

use g3_io_ext::LimitedReadExt;
use g3_types::collection::{SelectiveVec, WeightedValue};
use g3_types::net::UpstreamAddr;

use super::header::{HeaderBuilder, KitexTTHeaderBuilder, ThriftTHeaderBuilder};
use super::{
    MultiplexTransfer, SimplexTransfer, ThriftTcpRequest, ThriftTcpResponse,
    ThriftTcpResponseError, ThriftTcpResponseLocalError,
};
use crate::module::socket::{AppendSocketArgs, SocketArgs};
use crate::opts::ProcArgs;
use crate::target::thrift::{AppendThriftArgs, ThriftGlobalArgs};

const ARG_CONNECTION_POOL: &str = "connection-pool";
const ARG_TARGET: &str = "target";
const ARG_FRAMED: &str = "framed";
const ARG_THRIFT_THEADER: &str = "thrift-theader";
const ARG_KITEX_TTHEADER: &str = "kitex-ttheader";
const ARG_INFO_KV: &str = "info-kv";
const ARG_INFO_INT_KV: &str = "info-int-kv";
const ARG_ACL_TOKEN_KV: &str = "acl-token-kv";
const ARG_CONNECT_TIMEOUT: &str = "connect-timeout";
const ARG_TIMEOUT: &str = "timeout";
const ARG_MULTIPLEX: &str = "multiplex";
const ARG_NO_KEEPALIVE: &str = "no-keepalive";

const ARG_GROUP_HEADER: &str = "header";

pub(super) struct ThriftTcpArgs {
    pub(super) global: ThriftGlobalArgs,
    pub(super) pool_size: Option<usize>,
    target: UpstreamAddr,
    pub(super) framed: bool,
    pub(super) header_builder: Option<HeaderBuilder>,
    pub(super) timeout: Duration,
    pub(super) connect_timeout: Duration,
    pub(super) multiplex: bool,
    pub(super) no_keepalive: bool,

    socket: SocketArgs,

    target_addrs: Option<SelectiveVec<WeightedValue<SocketAddr>>>,
}

impl ThriftTcpArgs {
    fn new(global_args: ThriftGlobalArgs, target: UpstreamAddr) -> Self {
        ThriftTcpArgs {
            global: global_args,
            pool_size: None,
            target,
            framed: false,
            header_builder: None,
            timeout: Duration::from_secs(5),
            connect_timeout: Duration::from_secs(10),
            multiplex: false,
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

    pub(super) async fn new_multiplex_connection(
        self: &Arc<Self>,
        proc_args: &ProcArgs,
    ) -> anyhow::Result<MultiplexTransfer> {
        let tcp_stream = self.new_tcp_connection(proc_args).await?;
        let local_addr = tcp_stream
            .local_addr()
            .map_err(|e| anyhow!("failed to get local address: {e:?}"))?;

        let (r, w) = tcp_stream.into_split();
        Ok(MultiplexTransfer::start(
            self.clone(),
            r,
            w,
            local_addr,
            self.timeout,
        ))
    }

    pub(super) async fn new_simplex_connection(
        self: &Arc<Self>,
        proc_args: &ProcArgs,
    ) -> anyhow::Result<SimplexTransfer> {
        let tcp_stream = self.new_tcp_connection(proc_args).await?;
        let local_addr = tcp_stream
            .local_addr()
            .map_err(|e| anyhow!("failed to get local address: {e:?}"))?;

        let (r, w) = tcp_stream.into_split();
        Ok(SimplexTransfer::new(self.clone(), r, w, local_addr))
    }

    pub(super) fn build_tcp_request(
        &self,
        seq_id: i32,
        payload: &[u8],
    ) -> anyhow::Result<ThriftTcpRequest> {
        let mut buf = Vec::with_capacity(1024);

        if let Some(header_builder) = &self.header_builder {
            let offsets =
                header_builder.build(self.global.request_builder.protocol(), seq_id, &mut buf)?;

            self.global
                .request_builder
                .build(seq_id, self.framed, payload, &mut buf)?;

            header_builder.update_length(offsets, &mut buf)?;
        } else {
            self.global
                .request_builder
                .build(seq_id, self.framed, payload, &mut buf)?;
        }

        Ok(ThriftTcpRequest { seq_id, buf })
    }

    pub(super) async fn read_tcp_response<R>(
        &self,
        reader: &mut R,
        buf: &mut Vec<u8>,
    ) -> Result<ThriftTcpResponse, ThriftTcpResponseError>
    where
        R: AsyncRead + Unpin,
    {
        buf.resize(1024, 0);
        let nr = reader
            .read_all_once(buf)
            .await
            .map_err(ThriftTcpResponseLocalError::ReadFailed)?;

        println!("{nr} bytes received");

        Ok(ThriftTcpResponse { seq_id: 0 })
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
        Arg::new(ARG_THRIFT_THEADER)
            .help("Use Thrift THEADER")
            .long(ARG_THRIFT_THEADER)
            .action(ArgAction::SetTrue)
            .conflicts_with(ARG_KITEX_TTHEADER),
    )
    .arg(
        Arg::new(ARG_KITEX_TTHEADER)
            .help("Use Kitex TTHEADER")
            .long(ARG_KITEX_TTHEADER)
            .action(ArgAction::SetTrue)
            .conflicts_with(ARG_THRIFT_THEADER),
    )
    .group(ArgGroup::new(ARG_GROUP_HEADER).args([ARG_THRIFT_THEADER, ARG_KITEX_TTHEADER]))
    .arg(
        Arg::new(ARG_INFO_KV)
            .help("Set INFO_KEYVALUE to header")
            .long(ARG_INFO_KV)
            .num_args(1)
            .action(ArgAction::Append)
            .requires(ARG_GROUP_HEADER),
    )
    .arg(
        Arg::new(ARG_INFO_INT_KV)
            .help("Set INFO_INTKEYVALUE to kitex ttheader")
            .long(ARG_INFO_INT_KV)
            .num_args(1)
            .action(ArgAction::Append)
            .requires(ARG_KITEX_TTHEADER),
    )
    .arg(
        Arg::new(ARG_ACL_TOKEN_KV)
            .help("Set ACL_TOKEN_KEYVALUE to kitex ttheader")
            .long(ARG_ACL_TOKEN_KV)
            .num_args(1)
            .action(ArgAction::Append)
            .requires(ARG_KITEX_TTHEADER),
    )
    .arg(
        Arg::new(ARG_FRAMED)
            .help("Use framed transport")
            .long(ARG_FRAMED)
            .action(ArgAction::SetTrue),
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
        Arg::new(ARG_MULTIPLEX)
            .help("Use multiplexed transport")
            .action(ArgAction::SetTrue)
            .long(ARG_MULTIPLEX)
            .requires(ARG_GROUP_HEADER),
    )
    .arg(
        Arg::new(ARG_CONNECTION_POOL)
            .help(
                "Set the number of pooled underlying thrift connections.\n\
                        If not set, each concurrency will use it's own thrift connection",
            )
            .value_name("POOL SIZE")
            .long(ARG_CONNECTION_POOL)
            .short('C')
            .num_args(1)
            .value_parser(value_parser!(usize))
            .requires(ARG_MULTIPLEX),
    )
    .arg(
        Arg::new(ARG_NO_KEEPALIVE)
            .help("Disable keepalive on simplex connections")
            .action(ArgAction::SetTrue)
            .long(ARG_NO_KEEPALIVE)
            .conflicts_with(ARG_MULTIPLEX),
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

    t_args.framed = args.get_flag(ARG_FRAMED);

    if args.get_flag(ARG_KITEX_TTHEADER) {
        let mut builder = KitexTTHeaderBuilder::new(t_args.framed, &t_args.global.method)
            .context("failed to create Kitex TTHEADER builder")?;

        if let Some(values) = args.get_many::<String>(ARG_INFO_KV) {
            for value in values {
                let Some((k, v)) = value.split_once(':') else {
                    return Err(anyhow!("invalid INFO_KEYVALUE {value}"));
                };
                builder
                    .add_info_kv(k.trim(), v.trim())
                    .context(format!("invalid INFO_KEYVALUE {value}"))?;
            }
        }

        if let Some(values) = args.get_many::<String>(ARG_INFO_INT_KV) {
            for value in values {
                let Some((k, v)) = value.split_once(':') else {
                    return Err(anyhow!("invalid INFO_INTKEYVALUE {value}"));
                };
                let Ok(k) = u16::from_str(k) else {
                    return Err(anyhow!("invalid INFO_INTKEYVALUE {value}"));
                };
                builder
                    .add_info_int_kv(k, v.trim())
                    .context(format!("invalid INFO_INTKEYVALUE {value}"))?;
            }
        }

        if let Some(values) = args.get_many::<String>(ARG_ACL_TOKEN_KV) {
            for value in values {
                let Some((k, v)) = value.split_once(':') else {
                    return Err(anyhow!("invalid ACL_TOKEN_KEYVALUE {value}"));
                };
                builder
                    .add_acl_token_kv(k.trim(), v.trim())
                    .context(format!("invalid ACL_TOKEN_KEYVALUE {value}"))?;
            }
        }

        t_args.header_builder = Some(HeaderBuilder::Kitex(builder));
    } else if args.get_flag(ARG_THRIFT_THEADER) {
        let mut builder = ThriftTHeaderBuilder::default();

        if let Some(values) = args.get_many::<String>(ARG_INFO_KV) {
            for value in values {
                let Some((k, v)) = value.split_once(':') else {
                    return Err(anyhow!("invalid INFO_KEYVALUE {value}"));
                };
                builder
                    .add_info_kv(k.trim(), v.trim())
                    .context(format!("invalid INFO_KEYVALUE {value}"))?;
            }
        }

        t_args.header_builder = Some(HeaderBuilder::Thrift(builder));
    }

    t_args.multiplex = args.get_flag(ARG_MULTIPLEX);
    if let Some(c) = args.get_one::<usize>(ARG_CONNECTION_POOL) {
        if *c > 0 {
            t_args.pool_size = Some(*c);
        }
    }

    t_args.no_keepalive = args.get_flag(ARG_NO_KEEPALIVE);

    if let Some(timeout) = g3_clap::humanize::get_duration(args, ARG_CONNECT_TIMEOUT)? {
        t_args.connect_timeout = timeout;
    }
    if let Some(timeout) = g3_clap::humanize::get_duration(args, ARG_TIMEOUT)? {
        t_args.timeout = timeout;
    }

    t_args
        .socket
        .parse_args(args)
        .context("invalid socket config")?;

    Ok(t_args)
}
