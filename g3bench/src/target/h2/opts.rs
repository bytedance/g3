/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, anyhow};
use bytes::Bytes;
use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};
use h2::client::SendRequest;
use http::{HeaderValue, Method, StatusCode};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use url::Url;

use g3_io_ext::LimitedStream;
use g3_openssl::SslStream;
use g3_types::collection::{SelectiveVec, WeightedValue};
use g3_types::net::{
    AlpnProtocol, HttpAuth, OpensslClientConfig, OpensslClientConfigBuilder, Proxy, UpstreamAddr,
};

use super::{H2PreRequest, HttpRuntimeStats, ProcArgs};
use crate::module::openssl::{AppendOpensslArgs, OpensslTlsClientArgs};
use crate::module::proxy_protocol::{AppendProxyProtocolArgs, ProxyProtocolArgs};
use crate::module::socket::{AppendSocketArgs, SocketArgs};

const HTTP_ARG_CONNECTION_POOL: &str = "connection-pool";
const HTTP_ARG_URI: &str = "uri";
const HTTP_ARG_METHOD: &str = "method";
const HTTP_ARG_PROXY: &str = "proxy";
const HTTP_ARG_NO_MULTIPLEX: &str = "no-multiplex";
const HTTP_ARG_OK_STATUS: &str = "ok-status";
const HTTP_ARG_TIMEOUT: &str = "timeout";
const HTTP_ARG_CONNECT_TIMEOUT: &str = "connect-timeout";

pub(super) struct BenchH2Args {
    pub(super) pool_size: Option<usize>,
    pub(super) method: Method,
    target_url: Url,
    connect_proxy: Option<Proxy>,
    pub(super) no_multiplex: bool,
    pub(super) ok_status: Option<StatusCode>,
    pub(super) timeout: Duration,
    pub(super) connect_timeout: Duration,

    socket: SocketArgs,
    target_tls: OpensslTlsClientArgs,
    proxy_tls: OpensslTlsClientArgs,
    proxy_protocol: ProxyProtocolArgs,

    target: UpstreamAddr,
    auth: HttpAuth,
    peer_addrs: Option<SelectiveVec<WeightedValue<SocketAddr>>>,
}

impl BenchH2Args {
    fn new(url: Url) -> anyhow::Result<Self> {
        let upstream = UpstreamAddr::try_from(&url)?;
        let auth = HttpAuth::try_from(&url)
            .map_err(|e| anyhow!("failed to detect upstream auth method: {e}"))?;

        let mut target_tls = OpensslTlsClientArgs::default();
        if url.scheme() == "https" {
            target_tls.config = Some(OpensslClientConfigBuilder::with_cache_for_one_site());
            target_tls.alpn_protocol = Some(AlpnProtocol::Http2);
        }

        Ok(BenchH2Args {
            pool_size: None,
            method: Method::GET,
            target_url: url,
            connect_proxy: None,
            no_multiplex: false,
            ok_status: None,
            timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(15),
            socket: SocketArgs::default(),
            target_tls,
            proxy_tls: OpensslTlsClientArgs::default(),
            proxy_protocol: ProxyProtocolArgs::default(),
            target: upstream,
            auth,
            peer_addrs: None,
        })
    }

    pub(super) async fn resolve_target_address(
        &mut self,
        proc_args: &ProcArgs,
    ) -> anyhow::Result<()> {
        let host = if let Some(proxy) = &self.connect_proxy {
            proxy.peer()
        } else {
            &self.target
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
                .map_err(|e| anyhow!("failed to write proxy protocol data: {e:?}"))?;
        }

        Ok(stream)
    }

    pub(super) async fn new_h2_connection(
        &self,
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
                            &self.target,
                        )
                        .await
                        .map_err(|e| {
                            anyhow!("http connect to {} failed: {e}", http_proxy.peer())
                        })?;

                        self.connect_to_target(proc_args, buf_stream.into_inner(), stats)
                            .await
                    } else {
                        let mut buf_stream = BufReader::new(stream);

                        g3_http::connect::client::http_connect_to(
                            &mut buf_stream,
                            &http_proxy.auth,
                            &self.target,
                        )
                        .await
                        .map_err(|e| {
                            anyhow!("http connect to {} failed: {e}", http_proxy.peer())
                        })?;

                        self.connect_to_target(proc_args, buf_stream.into_inner(), stats)
                            .await
                    }
                }
                Proxy::Socks4(socks4_proxy) => {
                    let mut stream = self.new_tcp_connection(proc_args).await.context(format!(
                        "failed to connect to socks4 proxy {}",
                        socks4_proxy.peer()
                    ))?;

                    g3_socks::v4a::client::socks4a_connect_to(&mut stream, &self.target)
                        .await
                        .map_err(|e| {
                            anyhow!("socks4a connect to {} failed: {e}", socks4_proxy.peer())
                        })?;

                    self.connect_to_target(proc_args, stream, stats).await
                }
                Proxy::Socks5(socks5_proxy) => {
                    let mut stream = self.new_tcp_connection(proc_args).await.context(format!(
                        "failed to connect to socks5 proxy {}",
                        socks5_proxy.peer()
                    ))?;

                    g3_socks::v5::client::socks5_connect_to(
                        &mut stream,
                        &socks5_proxy.auth,
                        &self.target,
                    )
                    .await
                    .map_err(|e| {
                        anyhow!("socks5 connect to {} failed: {e}", socks5_proxy.peer())
                    })?;

                    self.connect_to_target(proc_args, stream, stats).await
                }
            }
        } else {
            let stream = self
                .new_tcp_connection(proc_args)
                .await
                .context(format!("failed to connect to target host {}", self.target))?;
            self.connect_to_target(proc_args, stream, stats).await
        }
    }

    async fn connect_to_target<S>(
        &self,
        proc_args: &ProcArgs,
        stream: S,
        stats: &Arc<HttpRuntimeStats>,
    ) -> anyhow::Result<SendRequest<Bytes>>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        if let Some(tls_client) = &self.target_tls.client {
            let tls_stream = self
                .tls_connect_to_target(tls_client, stream, stats)
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

    async fn tls_connect_to_target<S>(
        &self,
        tls_client: &OpensslClientConfig,
        stream: S,
        stats: &HttpRuntimeStats,
    ) -> anyhow::Result<SslStream<S>>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let tls_stream = self
            .target_tls
            .connect_target(tls_client, stream, &self.target)
            .await?;

        stats.target_ssl_session.add_total();
        if tls_stream.ssl().session_reused() {
            stats.target_ssl_session.add_reused();
        }

        if let Some(alpn) = tls_stream.ssl().selected_alpn_protocol() {
            if AlpnProtocol::from_selected(alpn) != Some(AlpnProtocol::Http2) {
                return Err(anyhow!("invalid returned alpn protocol: {:?}", alpn));
            }
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

    pub(super) fn build_pre_request_header(&self) -> anyhow::Result<H2PreRequest> {
        let path_and_query = if let Some(q) = self.target_url.query() {
            format!("{}?{q}", self.target_url.path())
        } else {
            self.target_url.path().to_string()
        };
        let uri = http::Uri::builder()
            .scheme(self.target_url.scheme())
            .authority(self.target.to_string())
            .path_and_query(path_and_query)
            .build()
            .map_err(|e| anyhow!("failed to build request: {e:?}"))?;

        let auth = match &self.auth {
            HttpAuth::None => None,
            HttpAuth::Basic(basic) => {
                let value = format!("Basic {}", basic.encoded_value());
                let value = HeaderValue::from_str(&value)
                    .map_err(|e| anyhow!("invalid auth value: {e:?}"))?;
                Some(value)
            }
        };

        Ok(H2PreRequest {
            method: self.method.clone(),
            uri,
            auth,
        })
    }
}

pub(super) fn add_h2_args(app: Command) -> Command {
    app.arg(Arg::new(HTTP_ARG_URI).required(true).num_args(1))
        .arg(
            Arg::new(HTTP_ARG_CONNECTION_POOL)
                .help(
                    "Set the number of pooled underlying h2 connections.\n\
                        If not set, each concurrency will use it's own h2 connection",
                )
                .value_name("POOL SIZE")
                .long(HTTP_ARG_CONNECTION_POOL)
                .short('C')
                .num_args(1)
                .value_parser(value_parser!(usize))
                .conflicts_with(HTTP_ARG_NO_MULTIPLEX),
        )
        .arg(
            Arg::new(HTTP_ARG_METHOD)
                .value_name("METHOD")
                .short('m')
                .long(HTTP_ARG_METHOD)
                .num_args(1)
                .value_parser(["GET", "HEAD"])
                .default_value("GET"),
        )
        .arg(
            Arg::new(HTTP_ARG_PROXY)
                .value_name("PROXY URL")
                .short('x')
                .help("Use a proxy")
                .long(HTTP_ARG_PROXY)
                .num_args(1)
                .value_name("PROXY URL"),
        )
        .arg(
            Arg::new(HTTP_ARG_NO_MULTIPLEX)
                .help("Disable h2 connection multiplexing")
                .action(ArgAction::SetTrue)
                .long(HTTP_ARG_NO_MULTIPLEX)
                .conflicts_with(HTTP_ARG_CONNECTION_POOL),
        )
        .arg(
            Arg::new(HTTP_ARG_OK_STATUS)
                .help("Only treat this status code as success")
                .value_name("STATUS CODE")
                .long(HTTP_ARG_OK_STATUS)
                .num_args(1)
                .value_parser(value_parser!(StatusCode)),
        )
        .arg(
            Arg::new(HTTP_ARG_TIMEOUT)
                .help("Http response timeout")
                .value_name("TIMEOUT DURATION")
                .default_value("30s")
                .long(HTTP_ARG_TIMEOUT)
                .num_args(1),
        )
        .arg(
            Arg::new(HTTP_ARG_CONNECT_TIMEOUT)
                .help("Timeout for connection to next peer")
                .value_name("TIMEOUT DURATION")
                .default_value("15s")
                .long(HTTP_ARG_CONNECT_TIMEOUT)
                .num_args(1),
        )
        .append_socket_args()
        .append_openssl_args()
        .append_proxy_openssl_args()
        .append_proxy_protocol_args()
}

pub(super) fn parse_h2_args(args: &ArgMatches) -> anyhow::Result<BenchH2Args> {
    let url = if let Some(v) = args.get_one::<String>(HTTP_ARG_URI) {
        Url::parse(v).context(format!("invalid {HTTP_ARG_URI} value"))?
    } else {
        return Err(anyhow!("no target url set"));
    };

    let mut h2_args = BenchH2Args::new(url)?;

    if let Some(c) = args.get_one::<usize>(HTTP_ARG_CONNECTION_POOL) {
        if *c > 0 {
            h2_args.pool_size = Some(*c);
        }
    }

    if let Some(v) = args.get_one::<String>(HTTP_ARG_METHOD) {
        let method = Method::from_str(v).context(format!("invalid {HTTP_ARG_METHOD} value"))?;
        h2_args.method = method;
    }

    if let Some(v) = args.get_one::<String>(HTTP_ARG_PROXY) {
        let url = Url::parse(v).context(format!("invalid {HTTP_ARG_PROXY} value"))?;
        let proxy = Proxy::try_from(&url).map_err(|e| anyhow!("invalid proxy: {e}"))?;
        if let Proxy::Http(mut http_proxy) = proxy {
            h2_args.proxy_tls.config = http_proxy.tls_config.take();
            h2_args.connect_proxy = Some(Proxy::Http(http_proxy));
        } else {
            h2_args.connect_proxy = Some(proxy);
        }
    }

    if args.get_flag(HTTP_ARG_NO_MULTIPLEX) {
        h2_args.no_multiplex = true;
    }

    if let Some(code) = args.get_one::<StatusCode>(HTTP_ARG_OK_STATUS) {
        h2_args.ok_status = Some(*code);
    }

    if let Some(timeout) = g3_clap::humanize::get_duration(args, HTTP_ARG_TIMEOUT)? {
        h2_args.timeout = timeout;
    }

    if let Some(timeout) = g3_clap::humanize::get_duration(args, HTTP_ARG_CONNECT_TIMEOUT)? {
        h2_args.connect_timeout = timeout;
    }

    h2_args
        .socket
        .parse_args(args)
        .context("invalid socket config")?;
    h2_args
        .target_tls
        .parse_tls_args(args)
        .context("invalid target tls config")?;
    h2_args
        .proxy_tls
        .parse_proxy_tls_args(args)
        .context("invalid proxy tls config")?;
    h2_args
        .proxy_protocol
        .parse_args(args)
        .context("invalid proxy protocol config")?;

    match h2_args.target_url.scheme() {
        "http" | "https" => {}
        _ => return Err(anyhow!("unsupported target url {}", h2_args.target_url)),
    }

    Ok(h2_args)
}
