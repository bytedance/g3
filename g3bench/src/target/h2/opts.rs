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
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context};
use bytes::Bytes;
use clap::{value_parser, Arg, ArgAction, ArgMatches, Command};
use h2::client::SendRequest;
use http::{HeaderValue, Method, StatusCode};
use openssl::ssl::SslVerifyMode;
use tokio::io::{AsyncRead, AsyncWrite, BufReader};
use tokio::net::TcpStream;
use tokio_openssl::SslStream;
use url::Url;

use g3_io_ext::{AggregatedIo, LimitedStream};
use g3_types::collection::{SelectiveVec, WeightedValue};
use g3_types::net::{
    AlpnProtocol, HttpAuth, OpensslTlsClientConfig, OpensslTlsClientConfigBuilder, Proxy,
    UpstreamAddr,
};

use super::{H2PreRequest, HttpRuntimeStats, ProcArgs};
use crate::target::{AppendTlsArgs, OpensslTlsClientArgs};

const HTTP_ARG_CONNECTION_POOL: &str = "connection-pool";
const HTTP_ARG_URI: &str = "uri";
const HTTP_ARG_METHOD: &str = "method";
const HTTP_ARG_PROXY: &str = "proxy";
const HTTP_ARG_LOCAL_ADDRESS: &str = "local-address";
const HTTP_ARG_NO_MULTIPLEX: &str = "no-multiplex";
const HTTP_ARG_OK_STATUS: &str = "ok-status";
const HTTP_ARG_TIMEOUT: &str = "timeout";
const HTTP_ARG_CONNECT_TIMEOUT: &str = "connect-timeout";

pub(super) struct BenchH2Args {
    pub(super) pool_size: Option<usize>,
    pub(super) method: Method,
    target_url: Url,
    connect_proxy: Option<Proxy>,
    bind: Option<IpAddr>,
    pub(super) no_multiplex: bool,
    pub(super) ok_status: Option<StatusCode>,
    pub(super) timeout: Duration,
    pub(super) connect_timeout: Duration,

    target_tls: OpensslTlsClientArgs,
    proxy_tls: OpensslTlsClientArgs,

    host: UpstreamAddr,
    auth: HttpAuth,
    peer_addrs: SelectiveVec<WeightedValue<SocketAddr>>,
}

impl BenchH2Args {
    fn new(url: Url) -> anyhow::Result<Self> {
        let upstream = UpstreamAddr::try_from(&url)?;
        let auth = HttpAuth::try_from(&url)
            .map_err(|e| anyhow!("failed to detect upstream auth method: {e}"))?;

        let mut target_tls = OpensslTlsClientArgs::default();
        if url.scheme() == "https" {
            target_tls.config = Some(OpensslTlsClientConfigBuilder::with_cache_for_one_site());
        }

        Ok(BenchH2Args {
            pool_size: None,
            method: Method::GET,
            target_url: url,
            connect_proxy: None,
            bind: None,
            no_multiplex: false,
            ok_status: None,
            timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(15),
            target_tls,
            proxy_tls: OpensslTlsClientArgs::default(),
            host: upstream,
            auth,
            peer_addrs: SelectiveVec::empty(),
        })
    }

    pub(super) async fn resolve_target_address(
        &mut self,
        proc_args: &ProcArgs,
    ) -> anyhow::Result<()> {
        let host = if let Some(proxy) = &self.connect_proxy {
            proxy.peer()
        } else {
            &self.host
        };
        self.peer_addrs = proc_args.resolve(host).await?;
        Ok(())
    }

    pub(super) async fn new_tcp_connection(
        &self,
        proc_args: &ProcArgs,
    ) -> anyhow::Result<TcpStream> {
        let peer = *proc_args.select_peer(&self.peer_addrs);

        let socket = g3_socket::tcp::new_socket_to(
            peer.ip(),
            self.bind,
            &Default::default(),
            &Default::default(),
            true,
        )
        .map_err(|e| anyhow!("failed to setup socket to {peer}: {e:?}"))?;
        socket
            .connect(peer)
            .await
            .map_err(|e| anyhow!("connect to {peer} error: {e:?}"))
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
                            .tls_connect_to_proxy(tls_config, http_proxy.peer(), stream)
                            .await?;

                        let (r, mut w) = tokio::io::split(tls_stream);
                        let mut buf_r = BufReader::new(r);

                        g3_http::connect::client::http_connect_to(
                            &mut buf_r,
                            &mut w,
                            &http_proxy.auth,
                            &self.host,
                        )
                        .await
                        .map_err(|e| {
                            anyhow!("http connect to {} failed: {e}", http_proxy.peer())
                        })?;

                        let stream = AggregatedIo::new(buf_r.into_inner(), w);
                        self.connect_to_target(proc_args, stream, stats).await
                    } else {
                        let (r, mut w) = stream.into_split();
                        let mut buf_r = BufReader::new(r);

                        g3_http::connect::client::http_connect_to(
                            &mut buf_r,
                            &mut w,
                            &http_proxy.auth,
                            &self.host,
                        )
                        .await
                        .map_err(|e| {
                            anyhow!("http connect to {} failed: {e}", http_proxy.peer())
                        })?;

                        let stream = AggregatedIo::new(buf_r.into_inner(), w);
                        self.connect_to_target(proc_args, stream, stats).await
                    }
                }
                Proxy::Socks4(socks4_proxy) => {
                    let stream = self.new_tcp_connection(proc_args).await.context(format!(
                        "failed to connect to socks4 proxy {}",
                        socks4_proxy.peer()
                    ))?;
                    let (mut r, mut w) = stream.into_split();

                    g3_socks::v4a::client::socks4a_connect_to(&mut r, &mut w, &self.host)
                        .await
                        .map_err(|e| {
                            anyhow!("socks4a connect to {} failed: {e}", socks4_proxy.peer())
                        })?;

                    let stream = AggregatedIo::new(r, w);
                    self.connect_to_target(proc_args, stream, stats).await
                }
                Proxy::Socks5(socks5_proxy) => {
                    let stream = self.new_tcp_connection(proc_args).await.context(format!(
                        "failed to connect to socks5 proxy {}",
                        socks5_proxy.peer()
                    ))?;
                    let (mut r, mut w) = stream.into_split();

                    g3_socks::v5::client::socks5_connect_to(
                        &mut r,
                        &mut w,
                        &socks5_proxy.auth,
                        &self.host,
                    )
                    .await
                    .map_err(|e| {
                        anyhow!("socks5 connect to {} failed: {e}", socks5_proxy.peer())
                    })?;

                    let stream = AggregatedIo::new(r, w);
                    self.connect_to_target(proc_args, stream, stats).await
                }
            }
        } else {
            let stream = self
                .new_tcp_connection(proc_args)
                .await
                .context(format!("failed to connect to target host {}", self.host))?;
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
                .tls_connect_to_target(tls_client, stream)
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
        let stream = LimitedStream::new(
            stream,
            speed_limit.shift_millis,
            speed_limit.max_south,
            speed_limit.max_north,
            stats.clone(),
        );

        let mut client_builder = h2::client::Builder::new();
        client_builder.max_concurrent_streams(1).enable_push(false);
        let (h2s, h2s_connection) = h2::client::handshake(stream)
            .await
            .map_err(|e| anyhow!("h2 handshake failed: {e:?}"))?;
        tokio::spawn(async move {
            let _ = h2s_connection.await;
        });
        Ok(h2s)
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
            .target_tls
            .tls_name
            .as_ref()
            .map(|v| Cow::Borrowed(v.as_str()))
            .unwrap_or_else(|| self.host.host_str());
        let mut ssl = tls_client
            .build_ssl(&tls_name, self.host.port())
            .context("failed to build ssl context")?;
        if self.target_tls.no_verify {
            ssl.set_verify(SslVerifyMode::NONE);
        }
        let mut tls_stream = SslStream::new(ssl, stream)
            .map_err(|e| anyhow!("tls connect to {tls_name} failed: {e}"))?;
        Pin::new(&mut tls_stream)
            .connect()
            .await
            .map_err(|e| anyhow!("tls connect to {tls_name} failed: {e}"))?;
        if let Some(alpn) = tls_stream.ssl().selected_alpn_protocol() {
            if AlpnProtocol::from_buf(alpn) != Some(AlpnProtocol::Http2) {
                return Err(anyhow!("invalid returned alpn protocol: {:?}", alpn));
            }
        }
        Ok(tls_stream)
    }

    async fn tls_connect_to_proxy(
        &self,
        tls_client: &OpensslTlsClientConfig,
        peer: &UpstreamAddr,
        stream: TcpStream,
    ) -> anyhow::Result<SslStream<TcpStream>> {
        let tls_name = self
            .proxy_tls
            .tls_name
            .as_ref()
            .map(|v| Cow::Borrowed(v.as_str()))
            .unwrap_or_else(|| peer.host_str());
        let mut ssl = tls_client
            .build_ssl(&tls_name, peer.port())
            .context("failed to build ssl context")?;
        if self.proxy_tls.no_verify {
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

    pub(super) fn build_pre_request_header(&self) -> anyhow::Result<H2PreRequest> {
        let path_and_query = if let Some(q) = self.target_url.query() {
            format!("{}?{q}", self.target_url.path())
        } else {
            self.target_url.path().to_string()
        };
        let uri = http::Uri::builder()
            .scheme(self.target_url.scheme())
            .authority(self.host.to_string())
            .path_and_query(path_and_query)
            .build()
            .map_err(|e| anyhow!("failed to build request: {e:?}"))?;

        let host_str = self.host.to_string();
        let host =
            HeaderValue::from_str(&host_str).map_err(|e| anyhow!("invalid host value: {e:?}"))?;

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
            host,
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
            Arg::new(HTTP_ARG_LOCAL_ADDRESS)
                .value_name("LOCAL IP ADDRESS")
                .short('B')
                .long(HTTP_ARG_LOCAL_ADDRESS)
                .num_args(1)
                .value_parser(value_parser!(IpAddr)),
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
        .append_tls_args()
        .append_proxy_tls_args()
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
        h2_args.connect_proxy = Some(proxy);
    }

    if let Some(ip) = args.get_one::<IpAddr>(HTTP_ARG_LOCAL_ADDRESS) {
        h2_args.bind = Some(*ip);
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
        .target_tls
        .parse_tls_args(args)
        .context("invalid target tls config")?;
    h2_args
        .proxy_tls
        .parse_proxy_tls_args(args)
        .context("invalid proxy tls config")?;

    match h2_args.target_url.scheme() {
        "http" | "https" => {}
        _ => return Err(anyhow!("unsupported target url {}", h2_args.target_url)),
    }

    Ok(h2_args)
}
