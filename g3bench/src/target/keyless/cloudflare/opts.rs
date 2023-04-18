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

use std::borrow::Cow;
use std::net::{IpAddr, SocketAddr};
use std::pin::Pin;
use std::time::Duration;

use anyhow::{anyhow, Context};
use clap::{value_parser, Arg, ArgMatches, Command};
use openssl::ssl::SslVerifyMode;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_openssl::SslStream;

use g3_types::collection::{SelectiveVec, WeightedValue};
use g3_types::net::{OpensslTlsClientConfig, OpensslTlsClientConfigBuilder, UpstreamAddr};

use super::SendHandle;
use crate::opts::ProcArgs;
use crate::target::keyless::{AppendKeylessArgs, KeylessGlobalArgs};
use crate::target::{
    AppendProxyProtocolArgs, AppendTlsArgs, OpensslTlsClientArgs, ProxyProtocolArgs,
};

const CF_ARG_CONNECTION_POOL: &str = "connection-pool";
const CF_ARG_TARGET: &str = "target";
const CF_ARG_LOCAL_ADDRESS: &str = "local-address";
const CF_ARG_CONNECT_TIMEOUT: &str = "connect-timeout";

pub(super) struct KeylessCloudflareArgs {
    pub(super) global: KeylessGlobalArgs,
    pub(super) pool_size: Option<usize>,
    target: UpstreamAddr,
    bind: Option<IpAddr>,
    pub(super) connect_timeout: Duration,
    pub(super) tls: OpensslTlsClientArgs,
    proxy_protocol: ProxyProtocolArgs,

    target_addrs: SelectiveVec<WeightedValue<SocketAddr>>,
}

impl KeylessCloudflareArgs {
    fn new(global_args: KeylessGlobalArgs, target: UpstreamAddr) -> Self {
        let tls = OpensslTlsClientArgs {
            config: Some(OpensslTlsClientConfigBuilder::with_cache_for_one_site()),
            ..Default::default()
        };
        KeylessCloudflareArgs {
            global: global_args,
            pool_size: None,
            target,
            bind: None,
            connect_timeout: Duration::from_secs(10),
            tls,
            proxy_protocol: ProxyProtocolArgs::default(),
            target_addrs: SelectiveVec::empty(),
        }
    }

    pub(super) async fn resolve_target_address(
        &mut self,
        proc_args: &ProcArgs,
    ) -> anyhow::Result<()> {
        self.target_addrs = proc_args.resolve(&self.target).await?;
        Ok(())
    }

    pub(super) async fn new_keyless_connection(
        &self,
        proc_args: &ProcArgs,
    ) -> anyhow::Result<SendHandle> {
        let tcp_stream = self.new_tcp_connection(proc_args).await?;
        if let Some(tls_client) = &self.tls.client {
            let ssl_stream = self.tls_connect_to_target(tls_client, tcp_stream).await?;
            let (r, w) = tokio::io::split(ssl_stream);
            Ok(super::connection::start_transfer(r, w))
        } else {
            let (r, w) = tcp_stream.into_split();
            Ok(super::connection::start_transfer(r, w))
        }
    }

    async fn new_tcp_connection(&self, proc_args: &ProcArgs) -> anyhow::Result<TcpStream> {
        let peer = *proc_args.select_peer(&self.target_addrs);

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

    async fn tls_connect_to_target<S>(
        &self,
        tls_client: &OpensslTlsClientConfig,
        stream: S,
    ) -> anyhow::Result<SslStream<S>>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let tls_name = self
            .tls
            .tls_name
            .as_ref()
            .map(|v| Cow::Borrowed(v.as_str()))
            .unwrap_or_else(|| self.target.host_str());
        let mut ssl = tls_client
            .build_ssl(&tls_name, self.target.port())
            .context("failed to build ssl context")?;
        if self.tls.no_verify {
            ssl.set_verify(SslVerifyMode::NONE);
        }
        let mut tls_stream = SslStream::new(ssl, stream)
            .map_err(|e| anyhow!("tls connect to {tls_name} failed: {e}"))?;
        Pin::new(&mut tls_stream)
            .connect()
            .await
            .map_err(|e| anyhow!("tls connect to {tls_name} failed: {e}"))?;
        Ok(tls_stream)
    }
}

pub(super) fn add_cloudflare_args(app: Command) -> Command {
    app.arg(
        Arg::new(CF_ARG_TARGET)
            .help("Target service address")
            .value_name("ADDRESS")
            .long(CF_ARG_TARGET)
            .required(true)
            .num_args(1)
            .value_parser(value_parser!(UpstreamAddr)),
    )
    .arg(
        Arg::new(CF_ARG_CONNECTION_POOL)
            .help(
                "Set the number of pooled underlying keyless connections.\n\
                        If not set, each concurrency will use it's own keyless connection",
            )
            .value_name("POOL SIZE")
            .long(CF_ARG_CONNECTION_POOL)
            .short('C')
            .num_args(1)
            .value_parser(value_parser!(usize)),
    )
    .arg(
        Arg::new(CF_ARG_LOCAL_ADDRESS)
            .value_name("LOCAL IP ADDRESS")
            .short('B')
            .long(CF_ARG_LOCAL_ADDRESS)
            .num_args(1)
            .value_parser(value_parser!(IpAddr)),
    )
    .arg(
        Arg::new(CF_ARG_CONNECT_TIMEOUT)
            .value_name("TIMEOUT DURATION")
            .help("Timeout for connection to next peer")
            .default_value("10s")
            .long(CF_ARG_CONNECT_TIMEOUT)
            .num_args(1),
    )
    .append_keyless_args()
    .append_tls_args()
    .append_proxy_protocol_args()
}

pub(super) fn parse_cloudflare_args(args: &ArgMatches) -> anyhow::Result<KeylessCloudflareArgs> {
    let target = if let Some(v) = args.get_one::<UpstreamAddr>(CF_ARG_TARGET) {
        v.clone()
    } else {
        return Err(anyhow!("no target set"));
    };

    let global_args =
        KeylessGlobalArgs::parse_args(args).context("failed to parse global keyless args")?;

    let mut cf_args = KeylessCloudflareArgs::new(global_args, target);

    if let Some(c) = args.get_one::<usize>(CF_ARG_CONNECTION_POOL) {
        if *c > 0 {
            cf_args.pool_size = Some(*c);
        }
    }

    if let Some(ip) = args.get_one::<IpAddr>(CF_ARG_LOCAL_ADDRESS) {
        cf_args.bind = Some(*ip);
    }

    if let Some(timeout) = g3_clap::humanize::get_duration(args, CF_ARG_CONNECT_TIMEOUT)? {
        cf_args.connect_timeout = timeout;
    }

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
