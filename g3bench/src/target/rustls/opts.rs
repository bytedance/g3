/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;
use std::time::Duration;

use anyhow::{Context, anyhow};
use clap::{Arg, ArgMatches, Command, value_parser};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::client::TlsStream;

use g3_types::collection::{SelectiveVec, WeightedValue};
use g3_types::net::{RustlsClientConfig, RustlsClientConfigBuilder, UpstreamAddr};

use super::ProcArgs;
use crate::module::proxy_protocol::{AppendProxyProtocolArgs, ProxyProtocolArgs};
use crate::module::rustls::{AppendRustlsArgs, RustlsTlsClientArgs};
use crate::module::socket::{AppendSocketArgs, SocketArgs};

const SSL_ARG_TARGET: &str = "target";
const SSL_ARG_TIMEOUT: &str = "timeout";
const SSL_ARG_CONNECT_TIMEOUT: &str = "connect-timeout";

pub(super) struct BenchRustlsArgs {
    target: UpstreamAddr,
    pub(super) timeout: Duration,
    pub(super) connect_timeout: Duration,

    socket: SocketArgs,
    pub(super) tls: RustlsTlsClientArgs,
    proxy_protocol: ProxyProtocolArgs,

    target_addrs: Option<SelectiveVec<WeightedValue<SocketAddr>>>,
}

impl BenchRustlsArgs {
    fn new(target: UpstreamAddr) -> Self {
        let tls = RustlsTlsClientArgs {
            config: Some(RustlsClientConfigBuilder::default()),
            ..Default::default()
        };
        BenchRustlsArgs {
            target,
            timeout: Duration::from_secs(10),
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

    pub(super) async fn new_tcp_connection(
        &self,
        proc_args: &ProcArgs,
    ) -> anyhow::Result<TcpStream> {
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

    pub(super) async fn tls_connect_to_target<S>(
        &self,
        tls_client: &RustlsClientConfig,
        stream: S,
    ) -> anyhow::Result<TlsStream<S>>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        self.tls
            .connect_target(tls_client, stream, &self.target)
            .await
    }
}

pub(super) fn add_ssl_args(app: Command) -> Command {
    app.arg(
        Arg::new(SSL_ARG_TARGET)
            .required(true)
            .num_args(1)
            .value_parser(value_parser!(UpstreamAddr)),
    )
    .arg(
        Arg::new(SSL_ARG_TIMEOUT)
            .value_name("TIMEOUT DURATION")
            .help("TLS handshake timeout")
            .default_value("10s")
            .long(SSL_ARG_TIMEOUT)
            .num_args(1),
    )
    .arg(
        Arg::new(SSL_ARG_CONNECT_TIMEOUT)
            .value_name("TIMEOUT DURATION")
            .help("Timeout for connection to next peer")
            .default_value("10s")
            .long(SSL_ARG_CONNECT_TIMEOUT)
            .num_args(1),
    )
    .append_socket_args()
    .append_rustls_args()
    .append_proxy_protocol_args()
}

pub(super) fn parse_ssl_args(args: &ArgMatches) -> anyhow::Result<BenchRustlsArgs> {
    let target = if let Some(v) = args.get_one::<UpstreamAddr>(SSL_ARG_TARGET) {
        v.clone()
    } else {
        return Err(anyhow!("no target set"));
    };

    let mut ssl_args = BenchRustlsArgs::new(target);

    if let Some(timeout) = g3_clap::humanize::get_duration(args, SSL_ARG_TIMEOUT)? {
        ssl_args.timeout = timeout;
    }

    if let Some(timeout) = g3_clap::humanize::get_duration(args, SSL_ARG_CONNECT_TIMEOUT)? {
        ssl_args.connect_timeout = timeout;
    }

    ssl_args
        .socket
        .parse_args(args)
        .context("invalid socket config")?;
    ssl_args
        .tls
        .parse_tls_args(args)
        .context("invalid tls config")?;
    ssl_args
        .proxy_protocol
        .parse_args(args)
        .context("invalid proxy protocol config")?;

    Ok(ssl_args)
}
