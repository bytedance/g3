/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;

use anyhow::{Context, anyhow};
use clap::{Arg, ArgAction, ArgMatches, Command};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use url::Url;

use g3_io_ext::{AsyncStream, LimitedReader, LimitedWriter};
use g3_openssl::SslStream;
use g3_types::collection::{SelectiveVec, WeightedValue};
use g3_types::net::{
    HttpProxy, OpensslClientConfig, OpensslClientConfigBuilder, Proxy, UpstreamAddr,
};

use crate::ProcArgs;
use crate::module::http::HttpRuntimeStats;
use crate::module::openssl::{AppendOpensslArgs, OpensslTlsClientArgs};
use crate::module::proxy_protocol::{AppendProxyProtocolArgs, ProxyProtocolArgs};
use crate::module::socket::{AppendSocketArgs, SocketArgs};

const HTTP_ARG_PROXY: &str = "proxy";
const HTTP_ARG_PROXY_TUNNEL: &str = "proxy-tunnel";

pub(crate) type BoxHttpForwardWriter = Box<dyn AsyncWrite + Send + Sync + Unpin>;
pub(crate) type BoxHttpForwardReader = Box<dyn AsyncRead + Send + Sync + Unpin>;
pub(crate) type BoxHttpForwardConnection = (BoxHttpForwardReader, BoxHttpForwardWriter);

pub(crate) struct SavedHttpForwardConnection {
    pub(crate) reader: BufReader<LimitedReader<BoxHttpForwardReader>>,
    pub(crate) writer: LimitedWriter<BoxHttpForwardWriter>,
}

impl SavedHttpForwardConnection {
    pub(crate) fn new(
        reader: BufReader<LimitedReader<BoxHttpForwardReader>>,
        writer: LimitedWriter<BoxHttpForwardWriter>,
    ) -> Self {
        SavedHttpForwardConnection { reader, writer }
    }
}

pub(crate) trait AppendH1ConnectArgs {
    fn append_h1_connect_args(self) -> Self;
}

pub(crate) struct H1ConnectArgs {
    pub(crate) forward_proxy: Option<HttpProxy>,
    connect_proxy: Option<Proxy>,

    socket: SocketArgs,
    target_tls: OpensslTlsClientArgs,
    proxy_tls: OpensslTlsClientArgs,
    proxy_protocol: ProxyProtocolArgs,

    peer_addrs: Option<SelectiveVec<WeightedValue<SocketAddr>>>,
}

impl H1ConnectArgs {
    pub(crate) fn new(is_https: bool) -> Self {
        let mut target_tls = OpensslTlsClientArgs::default();
        if is_https {
            target_tls.config = Some(OpensslClientConfigBuilder::with_cache_for_one_site());
        }

        H1ConnectArgs {
            forward_proxy: None,
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
        let host = if let Some(proxy) = &self.connect_proxy {
            proxy.peer()
        } else if let Some(proxy) = &self.forward_proxy {
            proxy.peer()
        } else {
            target
        };
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
                .map_err(|e| anyhow!("failed to send proxy protocol data: {e:?}"))?;
        }

        Ok(stream)
    }

    async fn tls_connect_to_peer<S>(
        &self,
        target: &UpstreamAddr,
        tls_client: &OpensslClientConfig,
        stream: S,
        stats: &HttpRuntimeStats,
    ) -> anyhow::Result<BoxHttpForwardConnection>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + Sync + 'static,
    {
        let tls_stream = self
            .target_tls
            .connect_target(tls_client, stream, target)
            .await?;

        stats.target_ssl_session.add_total();
        if tls_stream.ssl().session_reused() {
            stats.target_ssl_session.add_reused();
        }

        let (r, w) = tls_stream.into_split();
        Ok((Box::new(r), Box::new(w)))
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

    pub(crate) async fn new_http_connection(
        &self,
        target: &UpstreamAddr,
        stats: &HttpRuntimeStats,
        proc_args: &ProcArgs,
    ) -> anyhow::Result<BoxHttpForwardConnection> {
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

                        if let Some(tls_client) = &self.target_tls.client {
                            self.tls_connect_to_peer(
                                target,
                                tls_client,
                                buf_stream.into_inner(),
                                stats,
                            )
                            .await
                        } else {
                            let (r, w) = buf_stream.into_inner().into_split();
                            Ok((Box::new(r), Box::new(w)))
                        }
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

                        if let Some(tls_client) = &self.target_tls.client {
                            self.tls_connect_to_peer(
                                target,
                                tls_client,
                                buf_stream.into_inner(),
                                stats,
                            )
                            .await
                        } else {
                            let (r, w) = buf_stream.into_inner().into_split();
                            Ok((Box::new(r), Box::new(w)))
                        }
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

                    if let Some(tls_client) = &self.target_tls.client {
                        self.tls_connect_to_peer(target, tls_client, stream, stats)
                            .await
                    } else {
                        let (r, w) = stream.into_split();
                        Ok((Box::new(r), Box::new(w)))
                    }
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

                    if let Some(tls_client) = &self.target_tls.client {
                        self.tls_connect_to_peer(target, tls_client, stream, stats)
                            .await
                    } else {
                        let (r, w) = stream.into_split();
                        Ok((Box::new(r), Box::new(w)))
                    }
                }
            }
        } else if let Some(proxy) = &self.forward_proxy {
            let stream = self
                .new_tcp_connection(proc_args)
                .await
                .context(format!("failed to connect to http proxy {}", proxy.peer()))?;

            if let Some(tls_client) = &self.proxy_tls.client {
                let tls_stream = self
                    .tls_connect_to_proxy(tls_client, proxy.peer(), stream, stats)
                    .await?;

                let (r, w) = tls_stream.into_split();
                Ok((Box::new(r), Box::new(w)))
            } else {
                let (r, w) = stream.into_split();
                Ok((Box::new(r), Box::new(w)))
            }
        } else {
            let stream = self
                .new_tcp_connection(proc_args)
                .await
                .context(format!("failed to connect to target host {target}"))?;

            if let Some(tls_client) = &self.target_tls.client {
                self.tls_connect_to_peer(target, tls_client, stream, stats)
                    .await
            } else {
                let (r, w) = stream.into_split();
                Ok((Box::new(r), Box::new(w)))
            }
        }
    }

    pub(crate) fn parse_args(&mut self, args: &ArgMatches) -> anyhow::Result<()> {
        if let Some(v) = args.get_one::<String>(HTTP_ARG_PROXY) {
            let url = Url::parse(v).context(format!("invalid {HTTP_ARG_PROXY} value"))?;
            let proxy = Proxy::try_from(&url).map_err(|e| anyhow!("invalid proxy: {e}"))?;
            if let Proxy::Http(mut http_proxy) = proxy {
                self.proxy_tls.config = http_proxy.tls_config.take();
                if args.get_flag(HTTP_ARG_PROXY_TUNNEL) {
                    self.connect_proxy = Some(Proxy::Http(http_proxy));
                } else {
                    self.forward_proxy = Some(http_proxy);
                }
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

impl AppendH1ConnectArgs for Command {
    fn append_h1_connect_args(self) -> Self {
        self.arg(
            Arg::new(HTTP_ARG_PROXY)
                .value_name("PROXY URL")
                .short('x')
                .help("use a proxy")
                .long(HTTP_ARG_PROXY)
                .num_args(1)
                .value_name("PROXY URL"),
        )
        .arg(
            Arg::new(HTTP_ARG_PROXY_TUNNEL)
                .short('p')
                .long(HTTP_ARG_PROXY_TUNNEL)
                .action(ArgAction::SetTrue)
                .help("Use tunnel if the proxy is an HTTP proxy"),
        )
        .append_socket_args()
        .append_openssl_args()
        .append_proxy_openssl_args()
        .append_proxy_protocol_args()
    }
}
