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

use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use anyhow::{anyhow, Context};
use clap::{value_parser, Arg, ArgMatches, Command};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;

use g3_openssl::SslStream;
use g3_types::collection::{SelectiveVec, WeightedValue};
use g3_types::net::{OpensslClientConfig, OpensslClientConfigBuilder, UpstreamAddr};

use super::ProcArgs;
use crate::target::{
    AppendOpensslArgs, AppendProxyProtocolArgs, OpensslTlsClientArgs, ProxyProtocolArgs,
};

const SSL_ARG_TARGET: &str = "target";
const SSL_ARG_LOCAL_ADDRESS: &str = "local-address";
const SSL_ARG_TIMEOUT: &str = "timeout";
const SSL_ARG_CONNECT_TIMEOUT: &str = "connect-timeout";

pub(super) struct BenchSslArgs {
    target: UpstreamAddr,
    bind: Option<IpAddr>,
    pub(super) timeout: Duration,
    pub(super) connect_timeout: Duration,
    pub(super) tls: OpensslTlsClientArgs,
    proxy_protocol: ProxyProtocolArgs,

    target_addrs: Option<SelectiveVec<WeightedValue<SocketAddr>>>,
}

impl BenchSslArgs {
    fn new(target: UpstreamAddr) -> Self {
        let tls = OpensslTlsClientArgs {
            config: Some(OpensslClientConfigBuilder::with_cache_for_one_site()),
            ..Default::default()
        };
        BenchSslArgs {
            target,
            bind: None,
            timeout: Duration::from_secs(10),
            connect_timeout: Duration::from_secs(10),
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

        let socket = g3_socket::tcp::new_socket_to(
            peer.ip(),
            self.bind,
            &Default::default(),
            &Default::default(),
            true,
        )
        .map_err(|e| anyhow!("failed to setup socket to peer {peer}: {e:?}"))?;
        let mut stream = socket
            .connect(peer)
            .await
            .map_err(|e| anyhow!("connect to {peer} error: {e:?}"))?;

        if let Some(data) = self.proxy_protocol.data() {
            stream
                .write_all(data)
                .await
                .map_err(|e| anyhow!("failed to write proxy protocol data: {e:?}"))?;
        }

        Ok(stream)
    }

    pub(super) async fn tls_connect_to_target<S>(
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

pub(super) fn add_ssl_args(app: Command) -> Command {
    app.arg(
        Arg::new(SSL_ARG_TARGET)
            .required(true)
            .num_args(1)
            .value_parser(value_parser!(UpstreamAddr)),
    )
    .arg(
        Arg::new(SSL_ARG_LOCAL_ADDRESS)
            .value_name("LOCAL IP ADDRESS")
            .short('B')
            .long(SSL_ARG_LOCAL_ADDRESS)
            .num_args(1)
            .value_parser(value_parser!(IpAddr)),
    )
    .arg(
        Arg::new(SSL_ARG_TIMEOUT)
            .value_name("TIMEOUT DURATION")
            .help("SSL handshake timeout")
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
    .append_openssl_args()
    .append_proxy_protocol_args()
}

pub(super) fn parse_ssl_args(args: &ArgMatches) -> anyhow::Result<BenchSslArgs> {
    let target = if let Some(v) = args.get_one::<UpstreamAddr>(SSL_ARG_TARGET) {
        v.clone()
    } else {
        return Err(anyhow!("no target set"));
    };

    let mut ssl_args = BenchSslArgs::new(target);

    if let Some(ip) = args.get_one::<IpAddr>(SSL_ARG_LOCAL_ADDRESS) {
        ssl_args.bind = Some(*ip);
    }

    if let Some(timeout) = g3_clap::humanize::get_duration(args, SSL_ARG_TIMEOUT)? {
        ssl_args.timeout = timeout;
    }

    if let Some(timeout) = g3_clap::humanize::get_duration(args, SSL_ARG_CONNECT_TIMEOUT)? {
        ssl_args.connect_timeout = timeout;
    }

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
