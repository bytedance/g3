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

use std::net::SocketAddr;
use std::time::Duration;

use anyhow::{anyhow, Context};
use clap::{value_parser, Arg, ArgAction, ArgMatches, Command};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;

use g3_io_ext::AsyncStream;
use g3_openssl::SslStream;
use g3_types::collection::{SelectiveVec, WeightedValue};
use g3_types::net::{OpensslClientConfig, OpensslClientConfigBuilder, UpstreamAddr};

use super::{MultiplexTransfer, SimplexTransfer};
use crate::module::openssl::{AppendOpensslArgs, OpensslTlsClientArgs};
use crate::module::proxy_protocol::{AppendProxyProtocolArgs, ProxyProtocolArgs};
use crate::module::socket::{AppendSocketArgs, SocketArgs};
use crate::opts::ProcArgs;
use crate::target::keyless::{AppendKeylessArgs, KeylessGlobalArgs};

const ARG_CONNECTION_POOL: &str = "connection-pool";
const ARG_TARGET: &str = "target";
const ARG_NO_TLS: &str = "no-tls";
const ARG_CONNECT_TIMEOUT: &str = "connect-timeout";
const ARG_TIMEOUT: &str = "timeout";
const ARG_NO_MULTIPLEX: &str = "no-multiplex";

pub(super) struct KeylessCloudflareArgs {
    pub(super) global: KeylessGlobalArgs,
    pub(super) pool_size: Option<usize>,
    target: UpstreamAddr,
    pub(super) no_multiplex: bool,
    pub(super) timeout: Duration,
    pub(super) connect_timeout: Duration,

    socket: SocketArgs,
    pub(super) tls: OpensslTlsClientArgs,
    proxy_protocol: ProxyProtocolArgs,

    target_addrs: Option<SelectiveVec<WeightedValue<SocketAddr>>>,
}

impl KeylessCloudflareArgs {
    fn new(global_args: KeylessGlobalArgs, target: UpstreamAddr, no_tls: bool) -> Self {
        let tls = if no_tls {
            OpensslTlsClientArgs::default()
        } else {
            OpensslTlsClientArgs {
                config: Some(OpensslClientConfigBuilder::with_cache_for_one_site()),
                ..Default::default()
            }
        };
        KeylessCloudflareArgs {
            global: global_args,
            pool_size: None,
            target,
            no_multiplex: false,
            timeout: Duration::from_secs(5),
            connect_timeout: Duration::from_secs(10),
            socket: SocketArgs::default(),
            tls,
            proxy_protocol: ProxyProtocolArgs::default(),
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

    pub(super) async fn new_multiplex_keyless_connection(
        &self,
        proc_args: &ProcArgs,
    ) -> anyhow::Result<MultiplexTransfer> {
        let tcp_stream = self.new_tcp_connection(proc_args).await?;
        let local_addr = tcp_stream
            .local_addr()
            .map_err(|e| anyhow!("failed to get local address: {e:?}"))?;
        if let Some(tls_client) = &self.tls.client {
            let ssl_stream = self.tls_connect_to_target(tls_client, tcp_stream).await?;
            let (r, w) = ssl_stream.into_split();
            Ok(MultiplexTransfer::start(r, w, local_addr, self.timeout))
        } else {
            let (r, w) = tcp_stream.into_split();
            Ok(MultiplexTransfer::start(r, w, local_addr, self.timeout))
        }
    }

    pub(super) async fn new_simplex_keyless_connection(
        &self,
        proc_args: &ProcArgs,
    ) -> anyhow::Result<SimplexTransfer> {
        let tcp_stream = self.new_tcp_connection(proc_args).await?;
        let local_addr = tcp_stream
            .local_addr()
            .map_err(|e| anyhow!("failed to get local address: {e:?}"))?;
        if let Some(tls_client) = &self.tls.client {
            let ssl_stream = self.tls_connect_to_target(tls_client, tcp_stream).await?;
            let (r, w) = ssl_stream.into_split();
            Ok(SimplexTransfer::new(r, w, local_addr))
        } else {
            let (r, w) = tcp_stream.into_split();
            Ok(SimplexTransfer::new(r, w, local_addr))
        }
    }

    async fn new_tcp_connection(&self, proc_args: &ProcArgs) -> anyhow::Result<TcpStream> {
        let addrs = self
            .target_addrs
            .as_ref()
            .ok_or_else(|| anyhow!("no target addr set"))?;
        let peer = *proc_args.select_peer(addrs);

        let mut stream = self.socket.tcp_connect_to(peer).await?;

        if let Some(data) = self.proxy_protocol.data() {
            stream
                .write_all(data) // no need to flush data
                .await
                .map_err(|e| anyhow!("failed to write proxy protocol data: {e:?}"))?;
        }

        Ok(stream)
    }

    async fn tls_connect_to_target<S>(
        &self,
        tls_client: &OpensslClientConfig,
        stream: S,
    ) -> anyhow::Result<SslStream<S>>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        self.tls
            .connect_target(tls_client, stream, &self.target)
            .await
    }
}

pub(super) fn add_cloudflare_args(app: Command) -> Command {
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
        Arg::new(ARG_NO_TLS)
            .help("Use no tls")
            .long(ARG_NO_TLS)
            .action(ArgAction::SetTrue)
            .num_args(0),
    )
    .arg(
        Arg::new(ARG_CONNECTION_POOL)
            .help(
                "Set the number of pooled underlying keyless connections.\n\
                        If not set, each concurrency will use it's own keyless connection",
            )
            .value_name("POOL SIZE")
            .long(ARG_CONNECTION_POOL)
            .short('C')
            .num_args(1)
            .value_parser(value_parser!(usize))
            .conflicts_with(ARG_NO_MULTIPLEX),
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
        Arg::new(ARG_NO_MULTIPLEX)
            .help("Disable multiplex usage on the connection")
            .long(ARG_NO_MULTIPLEX)
            .action(ArgAction::SetTrue)
            .num_args(0)
            .conflicts_with(ARG_CONNECTION_POOL),
    )
    .append_socket_args()
    .append_keyless_args()
    .append_openssl_args()
    .append_proxy_protocol_args()
}

pub(super) fn parse_cloudflare_args(args: &ArgMatches) -> anyhow::Result<KeylessCloudflareArgs> {
    let target = if let Some(v) = args.get_one::<UpstreamAddr>(ARG_TARGET) {
        v.clone()
    } else {
        return Err(anyhow!("no target set"));
    };
    let no_tls = args.get_flag(ARG_NO_TLS);

    let global_args =
        KeylessGlobalArgs::parse_args(args).context("failed to parse global keyless args")?;

    let mut cf_args = KeylessCloudflareArgs::new(global_args, target, no_tls);

    if let Some(c) = args.get_one::<usize>(ARG_CONNECTION_POOL) {
        if *c > 0 {
            cf_args.pool_size = Some(*c);
        }
    }

    if let Some(timeout) = g3_clap::humanize::get_duration(args, ARG_CONNECT_TIMEOUT)? {
        cf_args.connect_timeout = timeout;
    }
    if let Some(timeout) = g3_clap::humanize::get_duration(args, ARG_TIMEOUT)? {
        cf_args.timeout = timeout;
    }

    if args.get_flag(ARG_NO_MULTIPLEX) {
        cf_args.no_multiplex = true;
    }

    cf_args
        .socket
        .parse_args(args)
        .context("invalid socket config")?;
    cf_args
        .tls
        .parse_tls_args(args)
        .context("invalid tls config")?;
    cf_args
        .proxy_protocol
        .parse_args(args)
        .context("invalid proxy protocol config")?;

    Ok(cf_args)
}
