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
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context};
use bytes::Bytes;
use clap::{value_parser, Arg, ArgAction, ArgMatches, Command};
use h3::client::SendRequest;
use h3_quinn::OpenStreams;
use http::{HeaderValue, Method, StatusCode};
use quinn::{Endpoint, TokioRuntime};
use rustls::ServerName;
use tokio::net::TcpStream;
use url::Url;

use g3_io_ext::LimitedTokioRuntime;
use g3_socks::v5::Socks5UdpTokioRuntime;
use g3_types::collection::{SelectiveVec, WeightedValue};
use g3_types::net::{
    AlpnProtocol, HttpAuth, Proxy, RustlsClientConfigBuilder, Socks5Proxy, UpstreamAddr,
};

use super::{H3PreRequest, HttpRuntimeStats, ProcArgs};
use crate::module::rustls::{AppendRustlsArgs, RustlsTlsClientArgs};

const HTTP_ARG_CONNECTION_POOL: &str = "connection-pool";
const HTTP_ARG_URI: &str = "uri";
const HTTP_ARG_METHOD: &str = "method";
const HTTP_ARG_PROXY: &str = "proxy";
const HTTP_ARG_LOCAL_ADDRESS: &str = "local-address";
const HTTP_ARG_NO_MULTIPLEX: &str = "no-multiplex";
const HTTP_ARG_OK_STATUS: &str = "ok-status";
const HTTP_ARG_TIMEOUT: &str = "timeout";
const HTTP_ARG_CONNECT_TIMEOUT: &str = "connect-timeout";

pub(super) struct BenchH3Args {
    pub(super) pool_size: Option<usize>,
    pub(super) method: Method,
    target_url: Url,
    bind: Option<IpAddr>,
    socks_proxy: Option<Socks5Proxy>,
    pub(super) no_multiplex: bool,
    pub(super) ok_status: Option<StatusCode>,
    pub(super) timeout: Duration,
    pub(super) connect_timeout: Duration,

    target_tls: RustlsTlsClientArgs,

    host: UpstreamAddr,
    auth: HttpAuth,
    proxy_peer_addrs: Option<SelectiveVec<WeightedValue<SocketAddr>>>,
    quic_peer_addrs: Option<SelectiveVec<WeightedValue<SocketAddr>>>,
}

impl BenchH3Args {
    fn new(url: Url) -> anyhow::Result<Self> {
        let upstream = UpstreamAddr::try_from(&url)?;
        let auth = HttpAuth::try_from(&url)
            .map_err(|e| anyhow!("failed to detect upstream auth method: {e}"))?;

        let tls = RustlsTlsClientArgs {
            config: Some(RustlsClientConfigBuilder::default()),
            alpn_protocol: Some(AlpnProtocol::Http3),
            ..Default::default()
        };

        Ok(BenchH3Args {
            pool_size: None,
            method: Method::GET,
            target_url: url,
            bind: None,
            socks_proxy: None,
            no_multiplex: false,
            ok_status: None,
            timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(15),
            target_tls: tls,
            host: upstream,
            auth,
            proxy_peer_addrs: None,
            quic_peer_addrs: None,
        })
    }

    pub(super) async fn resolve_target_address(
        &mut self,
        proc_args: &ProcArgs,
    ) -> anyhow::Result<()> {
        if let Some(proxy) = &self.socks_proxy {
            let addrs = proc_args.resolve(proxy.peer()).await?;
            self.proxy_peer_addrs = Some(addrs);
        };
        let addrs = proc_args.resolve(&self.host).await?;
        self.quic_peer_addrs = Some(addrs);
        Ok(())
    }

    pub(super) async fn new_tcp_connection(&self, peer: SocketAddr) -> anyhow::Result<TcpStream> {
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

    async fn new_quic_endpoint(
        &self,
        stats: &Arc<HttpRuntimeStats>,
        proc_args: &ProcArgs,
        quic_peer: SocketAddr,
    ) -> anyhow::Result<Endpoint> {
        if let Some(socks5_proxy) = &self.socks_proxy {
            let proxy_addrs = self
                .proxy_peer_addrs
                .as_ref()
                .ok_or_else(|| anyhow!("no proxy addr set"))?;
            let peer = *proc_args.select_peer(proxy_addrs);

            let stream = self.new_tcp_connection(peer).await.context(format!(
                "failed to connect to socks5 proxy {}",
                socks5_proxy.peer()
            ))?;
            let (mut r, mut w) = stream.into_split();

            let socket = g3_socket::udp::new_std_socket_to(
                peer,
                self.bind,
                Default::default(),
                Default::default(),
            )
            .map_err(|e| anyhow!("failed to setup local udp socket: {e}"))?;

            let local_udp_addr = socket
                .local_addr()
                .map_err(|e| anyhow!("failed to get local addr of udp socket: {e}"))?;
            let peer_udp_addr = g3_socks::v5::client::socks5_udp_associate(
                &mut r,
                &mut w,
                &socks5_proxy.auth,
                local_udp_addr,
            )
            .await
            .map_err(|e| {
                anyhow!(
                    "socks5 udp associate to {} failed: {e}",
                    socks5_proxy.peer()
                )
            })?;

            socket.connect(peer_udp_addr).map_err(|e| {
                anyhow!("failed to connect local udp socket to {peer_udp_addr}: {e}")
            })?;

            let tcp_stream = r.reunite(w).unwrap();
            let limit = &proc_args.udp_sock_speed_limit;
            let runtime = LimitedTokioRuntime::new(
                Socks5UdpTokioRuntime::new(tcp_stream, quic_peer),
                limit.shift_millis,
                limit.max_north_packets,
                limit.max_north_bytes,
                limit.max_south_packets,
                limit.max_south_bytes,
                stats.clone(),
            );
            Endpoint::new(Default::default(), None, socket, Arc::new(runtime))
                .map_err(|e| anyhow!("failed to create quic endpoint: {e}"))
        } else {
            let socket = g3_socket::udp::new_std_socket_to(
                quic_peer,
                self.bind,
                Default::default(),
                Default::default(),
            )
            .map_err(|e| anyhow!("failed to setup local udp socket: {e}"))?;
            socket
                .connect(quic_peer)
                .map_err(|e| anyhow!("failed to connect local udp socket to {quic_peer}: {e}"))?;

            let limit = &proc_args.udp_sock_speed_limit;
            let runtime = LimitedTokioRuntime::new(
                TokioRuntime,
                limit.shift_millis,
                limit.max_north_packets,
                limit.max_north_bytes,
                limit.max_south_packets,
                limit.max_south_bytes,
                stats.clone(),
            );
            Endpoint::new(Default::default(), None, socket, Arc::new(runtime))
                .map_err(|e| anyhow!("failed to create quic endpoint: {e}"))
        }
    }

    async fn new_quic_connection(
        &self,
        stats: &Arc<HttpRuntimeStats>,
        proc_args: &ProcArgs,
    ) -> anyhow::Result<h3_quinn::Connection> {
        use quinn::{ClientConfig, TransportConfig, VarInt};

        let addrs = self
            .quic_peer_addrs
            .as_ref()
            .ok_or_else(|| anyhow!("no peer addr set"))?;
        let quic_peer = *proc_args.select_peer(addrs);
        let endpoint = self.new_quic_endpoint(stats, proc_args, quic_peer).await?;

        let Some(tls_client) = &self.target_tls.client else {
            unreachable!()
        };
        let mut transport = TransportConfig::default();
        // no remotely-initiated bidi streams is needed
        transport.max_concurrent_bidi_streams(VarInt::from_u32(0));
        // remotely-initiated uni streams is needed by QPACK headers as say in
        //   https://http3-explained.haxx.se/en/h3/h3-streams
        // transport.max_concurrent_uni_streams(VarInt::from_u32(0));
        // TODO add more transport settings
        let mut client_config = ClientConfig::new(tls_client.driver.clone());
        client_config.transport_config(Arc::new(transport));

        let tls_name = match &self.target_tls.tls_name {
            Some(ServerName::DnsName(domain)) => domain.as_ref().to_string(),
            Some(ServerName::IpAddress(ip)) => ip.to_string(),
            Some(_) => return Err(anyhow!("unsupported tls server name type")),
            None => self.host.to_string(),
        };
        let conn = endpoint
            .connect_with(client_config, quic_peer, &tls_name)
            .map_err(|e| anyhow!("failed to create quic client: {e}"))?
            .await
            .map_err(|e| anyhow!("failed to connect: {e}"))?;
        Ok(h3_quinn::Connection::new(conn))
    }

    pub(super) async fn new_h3_connection(
        &self,
        stats: &Arc<HttpRuntimeStats>,
        proc_args: &ProcArgs,
    ) -> anyhow::Result<SendRequest<OpenStreams, Bytes>> {
        let quic_conn = self.new_quic_connection(stats, proc_args).await?;

        let mut client_builder = h3::client::builder();
        // TODO add more client config
        let (mut driver, send_request) = client_builder
            .build(quic_conn)
            .await
            .map_err(|e| anyhow!("failed to create h3 connection: {e}"))?;
        tokio::spawn(async move {
            let _ = driver.wait_idle().await;
        });

        Ok(send_request)
    }

    pub(super) fn build_pre_request_header(&self) -> anyhow::Result<H3PreRequest> {
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

        let auth = match &self.auth {
            HttpAuth::None => None,
            HttpAuth::Basic(basic) => {
                let value = format!("Basic {}", basic.encoded_value());
                let value = HeaderValue::from_str(&value)
                    .map_err(|e| anyhow!("invalid auth value: {e:?}"))?;
                Some(value)
            }
        };

        Ok(H3PreRequest {
            method: self.method.clone(),
            uri,
            auth,
        })
    }
}

pub(super) fn add_h3_args(app: Command) -> Command {
    app.arg(Arg::new(HTTP_ARG_URI).required(true).num_args(1))
        .arg(
            Arg::new(HTTP_ARG_CONNECTION_POOL)
                .help(
                    "Set the number of pooled underlying h3 connections.\n\
                        If not set, each concurrency will use it's own h3 connection",
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
                .help("Disable h3 connection multiplexing")
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
        .append_rustls_args()
}

pub(super) fn parse_h3_args(args: &ArgMatches) -> anyhow::Result<BenchH3Args> {
    let url = if let Some(v) = args.get_one::<String>(HTTP_ARG_URI) {
        Url::parse(v).context(format!("invalid {HTTP_ARG_URI} value"))?
    } else {
        return Err(anyhow!("no target url set"));
    };

    let mut h3_args = BenchH3Args::new(url)?;

    if let Some(c) = args.get_one::<usize>(HTTP_ARG_CONNECTION_POOL) {
        if *c > 0 {
            h3_args.pool_size = Some(*c);
        }
    }

    if let Some(v) = args.get_one::<String>(HTTP_ARG_METHOD) {
        let method = Method::from_str(v).context(format!("invalid {HTTP_ARG_METHOD} value"))?;
        h3_args.method = method;
    }

    if let Some(v) = args.get_one::<String>(HTTP_ARG_PROXY) {
        let url = Url::parse(v).context(format!("invalid {HTTP_ARG_PROXY} value"))?;
        let proxy = Proxy::try_from(&url).map_err(|e| anyhow!("invalid proxy: {e}"))?;
        let Proxy::Socks5(proxy) = proxy else {
            return Err(anyhow!("unsupported proxy {v}"));
        };
        h3_args.socks_proxy = Some(proxy);
    }

    if let Some(ip) = args.get_one::<IpAddr>(HTTP_ARG_LOCAL_ADDRESS) {
        h3_args.bind = Some(*ip);
    }

    if args.get_flag(HTTP_ARG_NO_MULTIPLEX) {
        h3_args.no_multiplex = true;
    }

    if let Some(code) = args.get_one::<StatusCode>(HTTP_ARG_OK_STATUS) {
        h3_args.ok_status = Some(*code);
    }

    if let Some(timeout) = g3_clap::humanize::get_duration(args, HTTP_ARG_TIMEOUT)? {
        h3_args.timeout = timeout;
    }

    if let Some(timeout) = g3_clap::humanize::get_duration(args, HTTP_ARG_CONNECT_TIMEOUT)? {
        h3_args.connect_timeout = timeout;
    }

    h3_args
        .target_tls
        .parse_tls_args(args)
        .context("invalid target tls config")?;

    if h3_args.target_url.scheme() != "https" {
        return Err(anyhow!("unsupported target url {}", h3_args.target_url));
    }

    Ok(h3_args)
}
