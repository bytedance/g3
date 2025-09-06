/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, anyhow};
use bytes::Bytes;
use clap::{Arg, ArgMatches, Command};
use h2::client::SendRequest;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use url::Url;

use g3_io_ext::LimitedStream;
use g3_openssl::SslStream;
use g3_types::collection::{SelectiveVec, WeightedValue};
use g3_types::net::{
    AlpnProtocol, OpensslClientConfig, OpensslClientConfigBuilder, Proxy, UpstreamAddr,
};

use crate::ProcArgs;
use crate::module::http::HttpRuntimeStats;
use crate::module::openssl::{AppendOpensslArgs, OpensslTlsClientArgs};
use crate::module::proxy_protocol::{AppendProxyProtocolArgs, ProxyProtocolArgs};
use crate::module::socket::{AppendSocketArgs, SocketArgs};

const HTTP_ARG_PROXY: &str = "proxy";

pub(crate) trait AppendH2ConnectArgs {
    fn append_h2_connect_args(self) -> Self;
}

pub(crate) struct H2ConnectArgs {
    connect_proxy: Option<Proxy>,

    socket: SocketArgs,
    target_tls: OpensslTlsClientArgs,
    proxy_tls: OpensslTlsClientArgs,
    proxy_protocol: ProxyProtocolArgs,

    peer_addrs: Option<SelectiveVec<WeightedValue<SocketAddr>>>,
}

impl H2ConnectArgs {
    pub(crate) fn new(is_https: bool) -> Self {
        let mut target_tls = OpensslTlsClientArgs::default();
        if is_https {
            target_tls.config = Some(OpensslClientConfigBuilder::with_cache_for_one_site());
            target_tls.alpn_protocol = Some(AlpnProtocol::Http2);
        }

        H2ConnectArgs {
            connect_proxy: None,
            socket: SocketArgs::default(),
            target_tls,
            proxy_tls: OpensslTlsClientArgs::default(),
            proxy_protocol: ProxyProtocolArgs::default(),
            peer_addrs: None,
        }
    }

    pub(crate) async fn resolve_target_address(
        &mut self,
        proc_args: &ProcArgs,
        target: &UpstreamAddr,
    ) -> anyhow::Result<()> {
        let host = self
            .connect_proxy
            .as_ref()
            .map(|v| v.peer())
            .unwrap_or(target);
        let addrs = proc_args.resolve(host).await?;
        self.peer_addrs = Some(addrs);
        Ok(())
    }

    async fn new_tcp_connection(&self, proc_args: &ProcArgs) -> anyhow::Result<TcpStream> {
        let addrs = self
            .peer_addrs
            .as_ref()
            .ok_or_else(|| anyhow!("no peer address set"))?;
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
        target: &UpstreamAddr,
        tls_client: &OpensslClientConfig,
        stream: S,
        stats: &HttpRuntimeStats,
    ) -> anyhow::Result<SslStream<S>>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let tls_stream = self
            .target_tls
            .connect_target(tls_client, stream, target)
            .await?;

        stats.target_ssl_session.add_total();
        if tls_stream.ssl().session_reused() {
            stats.target_ssl_session.add_reused();
        }

        if let Some(alpn) = tls_stream.ssl().selected_alpn_protocol()
            && AlpnProtocol::from_selected(alpn) != Some(AlpnProtocol::Http2)
        {
            return Err(anyhow!("invalid returned alpn protocol: {:?}", alpn));
        }
        Ok(tls_stream)
    }

    async fn tls_connect_to_proxy(
        &self,
        tls_client: &OpensslClientConfig,
        peer: &UpstreamAddr,
        stream: TcpStream,
        stats: &HttpRuntimeStats,
    ) -> anyhow::Result<SslStream<TcpStream>> {
        let tls_stream = self
            .proxy_tls
            .connect_target(tls_client, stream, peer)
            .await?;

        stats.proxy_ssl_session.add_total();
        if tls_stream.ssl().session_reused() {
            stats.proxy_ssl_session.add_reused();
        }

        Ok(tls_stream)
    }

    async fn h2_handshake<S>(
        &self,
        proc_args: &ProcArgs,
        stream: S,
        stats: &Arc<HttpRuntimeStats>,
    ) -> anyhow::Result<SendRequest<Bytes>>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let speed_limit = &proc_args.tcp_sock_speed_limit;
        let stream = LimitedStream::local_limited(
            stream,
            speed_limit.shift_millis,
            speed_limit.max_south,
            speed_limit.max_north,
            stats.clone(),
        );

        let mut client_builder = h2::client::Builder::new();
        client_builder.max_concurrent_streams(0).enable_push(false);
        let (h2s, h2s_connection) = client_builder
            .handshake(stream)
            .await
            .map_err(|e| anyhow!("h2 handshake failed: {e:?}"))?;
        tokio::spawn(async move {
            let _ = h2s_connection.await;
        });
        Ok(h2s)
    }

    async fn connect_to_target<S>(
        &self,
        target: &UpstreamAddr,
        proc_args: &ProcArgs,
        stream: S,
        stats: &Arc<HttpRuntimeStats>,
    ) -> anyhow::Result<SendRequest<Bytes>>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        if let Some(tls_client) = &self.target_tls.client {
            let tls_stream = self
                .tls_connect_to_target(target, tls_client, stream, stats)
                .await
                .context("tls connect to target failed")?;
            self.h2_handshake(proc_args, tls_stream, stats)
                .await
                .context("h2 handshake failed")
        } else {
            self.h2_handshake(proc_args, stream, stats)
                .await
                .context("h2 handshake failed")
        }
    }

    pub(crate) async fn new_h2_connection(
        &self,
        target: &UpstreamAddr,
        stats: &Arc<HttpRuntimeStats>,
        proc_args: &ProcArgs,
    ) -> anyhow::Result<SendRequest<Bytes>> {
        if let Some(proxy) = &self.connect_proxy {
            match proxy {
                Proxy::Http(http_proxy) => {
                    let stream = self.new_tcp_connection(proc_args).await.context(format!(
                        "failed to connect to http proxy {}",
                        http_proxy.peer()
                    ))?;

                    if let Some(tls_config) = &self.proxy_tls.client {
                        let tls_stream = self
                            .tls_connect_to_proxy(tls_config, http_proxy.peer(), stream, stats)
                            .await?;

                        let mut buf_stream = BufReader::new(tls_stream);

                        g3_http::connect::client::http_connect_to(
                            &mut buf_stream,
                            &http_proxy.auth,
                            target,
                        )
                        .await
                        .map_err(|e| {
                            anyhow!("http connect to {} failed: {e}", http_proxy.peer())
                        })?;

                        self.connect_to_target(target, proc_args, buf_stream.into_inner(), stats)
                            .await
                    } else {
                        let mut buf_stream = BufReader::new(stream);

                        g3_http::connect::client::http_connect_to(
                            &mut buf_stream,
                            &http_proxy.auth,
                            target,
                        )
                        .await
                        .map_err(|e| {
                            anyhow!("http connect to {} failed: {e}", http_proxy.peer())
                        })?;

                        self.connect_to_target(target, proc_args, buf_stream.into_inner(), stats)
                            .await
                    }
                }
                Proxy::Socks4(socks4_proxy) => {
                    let mut stream = self.new_tcp_connection(proc_args).await.context(format!(
                        "failed to connect to socks4 proxy {}",
                        socks4_proxy.peer()
                    ))?;

                    g3_socks::v4a::client::socks4a_connect_to(&mut stream, target)
                        .await
                        .map_err(|e| {
                            anyhow!("socks4a connect to {} failed: {e}", socks4_proxy.peer())
                        })?;

                    self.connect_to_target(target, proc_args, stream, stats)
                        .await
                }
                Proxy::Socks5(socks5_proxy) => {
                    let mut stream = self.new_tcp_connection(proc_args).await.context(format!(
                        "failed to connect to socks5 proxy {}",
                        socks5_proxy.peer()
                    ))?;

                    g3_socks::v5::client::socks5_connect_to(
                        &mut stream,
                        &socks5_proxy.auth,
                        target,
                    )
                    .await
                    .map_err(|e| {
                        anyhow!("socks5 connect to {} failed: {e}", socks5_proxy.peer())
                    })?;

                    self.connect_to_target(target, proc_args, stream, stats)
                        .await
                }
            }
        } else {
            let stream = self
                .new_tcp_connection(proc_args)
                .await
                .context(format!("failed to connect to target host {target}"))?;
            self.connect_to_target(target, proc_args, stream, stats)
                .await
        }
    }

    pub(crate) fn parse_args(&mut self, args: &ArgMatches) -> anyhow::Result<()> {
        if let Some(v) = args.get_one::<String>(HTTP_ARG_PROXY) {
            let url = Url::parse(v).context(format!("invalid {HTTP_ARG_PROXY} value"))?;
            let proxy = Proxy::try_from(&url).map_err(|e| anyhow!("invalid proxy: {e}"))?;
            if let Proxy::Http(mut http_proxy) = proxy {
                self.proxy_tls.config = http_proxy.tls_config.take();
                self.connect_proxy = Some(Proxy::Http(http_proxy));
            } else {
                self.connect_proxy = Some(proxy);
            }
        }

        self.socket
            .parse_args(args)
            .context("invalid socket config")?;
        self.target_tls
            .parse_tls_args(args)
            .context("invalid target tls config")?;
        self.proxy_tls
            .parse_proxy_tls_args(args)
            .context("invalid proxy tls config")?;
        self.proxy_protocol
            .parse_args(args)
            .context("invalid proxy protocol config")?;
        Ok(())
    }
}

impl AppendH2ConnectArgs for Command {
    fn append_h2_connect_args(self) -> Self {
        self.arg(
            Arg::new(HTTP_ARG_PROXY)
                .value_name("PROXY URL")
                .short('x')
                .help("Use a proxy")
                .long(HTTP_ARG_PROXY)
                .num_args(1)
                .value_name("PROXY URL"),
        )
        .append_socket_args()
        .append_openssl_args()
        .append_proxy_openssl_args()
        .append_proxy_protocol_args()
    }
}
