/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::net::SocketAddr;

use anyhow::{Context, anyhow};
use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};
use http::{HeaderValue, Request, Version, header};
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use url::Url;

use g3_io_ext::AsyncStream;
use g3_openssl::SslStream;
use g3_types::collection::{SelectiveVec, WeightedValue};
use g3_types::net::{
    HttpAuth, HttpProxy, OpensslClientConfig, OpensslClientConfigBuilder, Proxy, UpstreamAddr,
};

use super::{BoxHttpForwardConnection, HttpRuntimeStats, ProcArgs};
use crate::module::http::{AppendHttpArgs, HttpClientArgs};
use crate::module::openssl::{AppendOpensslArgs, OpensslTlsClientArgs};
use crate::module::proxy_protocol::{AppendProxyProtocolArgs, ProxyProtocolArgs};
use crate::module::socket::{AppendSocketArgs, SocketArgs};

const HTTP_ARG_PROXY: &str = "proxy";
const HTTP_ARG_PROXY_TUNNEL: &str = "proxy-tunnel";
const HTTP_ARG_NO_KEEPALIVE: &str = "no-keepalive";
const HTTP_ARG_HEADER_SIZE: &str = "header-size";

pub(super) struct BenchHttpArgs {
    pub(super) common: HttpClientArgs,
    forward_proxy: Option<HttpProxy>,
    connect_proxy: Option<Proxy>,
    pub(super) no_keepalive: bool,
    pub(super) max_header_size: usize,

    socket: SocketArgs,
    target_tls: OpensslTlsClientArgs,
    proxy_tls: OpensslTlsClientArgs,
    proxy_protocol: ProxyProtocolArgs,

    peer_addrs: Option<SelectiveVec<WeightedValue<SocketAddr>>>,
}

impl BenchHttpArgs {
    fn new(common: HttpClientArgs) -> anyhow::Result<Self> {
        let mut target_tls = OpensslTlsClientArgs::default();
        if common.is_https() {
            target_tls.config = Some(OpensslClientConfigBuilder::with_cache_for_one_site());
        }

        Ok(BenchHttpArgs {
            common,
            forward_proxy: None,
            connect_proxy: None,
            no_keepalive: false,
            max_header_size: 4096,
            socket: SocketArgs::default(),
            target_tls,
            proxy_tls: OpensslTlsClientArgs::default(),
            proxy_protocol: ProxyProtocolArgs::default(),
            peer_addrs: None,
        })
    }

    pub(super) async fn resolve_target_address(
        &mut self,
        proc_args: &ProcArgs,
    ) -> anyhow::Result<()> {
        let host = if let Some(proxy) = &self.connect_proxy {
            proxy.peer()
        } else if let Some(proxy) = &self.forward_proxy {
            proxy.peer()
        } else {
            &self.common.target
        };
        let addrs = proc_args.resolve(host).await?;
        self.peer_addrs = Some(addrs);
        Ok(())
    }

    pub(super) async fn new_tcp_connection(
        &self,
        proc_args: &ProcArgs,
    ) -> anyhow::Result<TcpStream> {
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

    pub(super) async fn new_http_connection(
        &self,
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
                            &self.common.target,
                        )
                        .await
                        .map_err(|e| {
                            anyhow!("http connect to {} failed: {e}", http_proxy.peer())
                        })?;

                        if let Some(tls_client) = &self.target_tls.client {
                            self.tls_connect_to_peer(tls_client, buf_stream.into_inner(), stats)
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
                            &self.common.target,
                        )
                        .await
                        .map_err(|e| {
                            anyhow!("http connect to {} failed: {e}", http_proxy.peer())
                        })?;

                        if let Some(tls_client) = &self.target_tls.client {
                            self.tls_connect_to_peer(tls_client, buf_stream.into_inner(), stats)
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

                    g3_socks::v4a::client::socks4a_connect_to(&mut stream, &self.common.target)
                        .await
                        .map_err(|e| {
                            anyhow!("socks4a connect to {} failed: {e}", socks4_proxy.peer())
                        })?;

                    if let Some(tls_client) = &self.target_tls.client {
                        self.tls_connect_to_peer(tls_client, stream, stats).await
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
                        &self.common.target,
                    )
                    .await
                    .map_err(|e| {
                        anyhow!("socks5 connect to {} failed: {e}", socks5_proxy.peer())
                    })?;

                    if let Some(tls_client) = &self.target_tls.client {
                        self.tls_connect_to_peer(tls_client, stream, stats).await
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
            let stream = self.new_tcp_connection(proc_args).await.context(format!(
                "failed to connect to target host {}",
                self.common.target
            ))?;

            if let Some(tls_client) = &self.target_tls.client {
                self.tls_connect_to_peer(tls_client, stream, stats).await
            } else {
                let (r, w) = stream.into_split();
                Ok((Box::new(r), Box::new(w)))
            }
        }
    }

    async fn tls_connect_to_peer<S>(
        &self,
        tls_client: &OpensslClientConfig,
        stream: S,
        stats: &HttpRuntimeStats,
    ) -> anyhow::Result<BoxHttpForwardConnection>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let tls_stream = self
            .target_tls
            .connect_target(tls_client, stream, &self.common.target)
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

    fn write_request_line<W: io::Write>(&self, buf: &mut W, req: &Request<()>) -> io::Result<()> {
        write!(buf, "{} ", req.method())?;
        if self.forward_proxy.is_some() {
            write!(
                buf,
                "{}://{}",
                self.common.target_url.scheme(),
                self.common.target
            )?;
        }
        match req.uri().path_and_query() {
            Some(v) => {
                buf.write_all(v.as_str().as_bytes())?;
            }
            None => {
                buf.write_all(b"/")?;
            }
        }
        buf.write_all(b" HTTP/1.1\r\n")?;

        Ok(())
    }

    pub(super) fn write_fixed_request_header<W: io::Write>(
        &self,
        buf: &mut W,
    ) -> anyhow::Result<()> {
        let mut static_request = self.common.build_static_request(Version::HTTP_11)?;

        if !static_request.headers().contains_key(header::HOST) {
            let v = HeaderValue::from_str(&self.common.target.to_string())?;
            static_request.headers_mut().insert(header::HOST, v);
        }

        if let Some(p) = &self.forward_proxy {
            match &p.auth {
                HttpAuth::None => {}
                HttpAuth::Basic(basic) => {
                    if !static_request
                        .headers()
                        .contains_key(header::PROXY_AUTHORIZATION)
                    {
                        let value = HeaderValue::try_from(basic)
                            .map_err(|e| anyhow!("invalid auth value: {e:?}"))?;
                        static_request
                            .headers_mut()
                            .insert(header::PROXY_AUTHORIZATION, value);
                    }
                }
            }
        }

        if self.no_keepalive {
            static_request
                .headers_mut()
                .insert(header::CONNECTION, HeaderValue::from_static("close"));
        } else {
            static_request
                .headers_mut()
                .insert(header::CONNECTION, HeaderValue::from_static("keep-alive"));
        }

        self.write_request_line(buf, &static_request)?;

        for (k, v) in static_request.headers() {
            buf.write_all(k.as_str().as_bytes())?;
            buf.write_all(b": ")?;
            buf.write_all(v.as_bytes())?;
            buf.write_all(b"\r\n")?;
        }

        Ok(())
    }
}

pub(super) fn add_http_args(app: Command) -> Command {
    app.arg(
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
    .arg(
        Arg::new(HTTP_ARG_NO_KEEPALIVE)
            .help("Disable http keepalive")
            .action(ArgAction::SetTrue)
            .long(HTTP_ARG_NO_KEEPALIVE),
    )
    .arg(
        Arg::new(HTTP_ARG_HEADER_SIZE)
            .value_name("SIZE")
            .help("Set max response header size")
            .long(HTTP_ARG_HEADER_SIZE)
            .num_args(1)
            .value_parser(value_parser!(usize)),
    )
    .append_http_args()
    .append_socket_args()
    .append_openssl_args()
    .append_proxy_openssl_args()
    .append_proxy_protocol_args()
}

pub(super) fn parse_http_args(args: &ArgMatches) -> anyhow::Result<BenchHttpArgs> {
    let common = HttpClientArgs::parse_http_args(args)?;
    let mut h1_args = BenchHttpArgs::new(common)?;

    if let Some(v) = args.get_one::<String>(HTTP_ARG_PROXY) {
        let url = Url::parse(v).context(format!("invalid {HTTP_ARG_PROXY} value"))?;
        let proxy = Proxy::try_from(&url).map_err(|e| anyhow!("invalid proxy: {e}"))?;
        if let Proxy::Http(mut http_proxy) = proxy {
            h1_args.proxy_tls.config = http_proxy.tls_config.take();
            if args.get_flag(HTTP_ARG_PROXY_TUNNEL) {
                h1_args.connect_proxy = Some(Proxy::Http(http_proxy));
            } else {
                h1_args.forward_proxy = Some(http_proxy);
            }
        } else {
            h1_args.connect_proxy = Some(proxy);
        }
    }

    if args.get_flag(HTTP_ARG_NO_KEEPALIVE) {
        h1_args.no_keepalive = true;
    }

    if let Some(header_size) = g3_clap::humanize::get_usize(args, HTTP_ARG_HEADER_SIZE)? {
        h1_args.max_header_size = header_size;
    }

    h1_args
        .socket
        .parse_args(args)
        .context("invalid socket config")?;
    h1_args
        .target_tls
        .parse_tls_args(args)
        .context("invalid target tls config")?;
    h1_args
        .proxy_tls
        .parse_proxy_tls_args(args)
        .context("invalid proxy tls config")?;
    h1_args
        .proxy_protocol
        .parse_args(args)
        .context("invalid proxy protocol config")?;

    match h1_args.common.target_url.scheme() {
        "http" | "https" => {}
        "ftp" => {
            if h1_args.forward_proxy.is_none() {
                return Err(anyhow!(
                    "forward proxy is required for target url {}",
                    h1_args.common.target_url
                ));
            }
        }
        _ => {
            return Err(anyhow!(
                "unsupported target url {}",
                h1_args.common.target_url
            ));
        }
    }

    Ok(h1_args)
}
