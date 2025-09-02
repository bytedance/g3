/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use std::net::{IpAddr, SocketAddr};
use std::sync::Arc;

use anyhow::{Context, anyhow};
use bytes::Bytes;
use clap::{Arg, ArgMatches, Command};
use h3::client::SendRequest;
use h3_quinn::{OpenStreams, VarInt};
use quinn::crypto::rustls::QuicClientConfig;
use quinn::{ClientConfig, Endpoint, TokioRuntime, TransportConfig};
use rustls_pki_types::ServerName;
use url::Url;

use g3_io_ext::LimitedTokioRuntime;
use g3_socks::v5::Socks5UdpTokioRuntime;
use g3_types::collection::{SelectiveVec, WeightedValue};
use g3_types::net::{AlpnProtocol, Proxy, RustlsClientConfigBuilder, Socks5Proxy, UpstreamAddr};

use crate::ProcArgs;
use crate::module::http::HttpRuntimeStats;
use crate::module::rustls::{AppendRustlsArgs, RustlsTlsClientArgs};
use crate::module::socket::{AppendSocketArgs, SocketArgs};

const HTTP_ARG_PROXY: &str = "proxy";

pub(crate) trait AppendH3ConnectArgs {
    fn append_h3_connect_args(self) -> Self;
}

pub(crate) struct H3ConnectArgs {
    socks_proxy: Option<Socks5Proxy>,

    socket: SocketArgs,
    target_tls: RustlsTlsClientArgs,

    proxy_peer_addrs: Option<SelectiveVec<WeightedValue<SocketAddr>>>,
    quic_peer_addrs: Option<SelectiveVec<WeightedValue<SocketAddr>>>,
}

impl Default for H3ConnectArgs {
    fn default() -> Self {
        H3ConnectArgs::new()
    }
}

impl H3ConnectArgs {
    pub(crate) fn new() -> Self {
        let tls = RustlsTlsClientArgs {
            config: Some(RustlsClientConfigBuilder::default()),
            alpn_protocol: Some(AlpnProtocol::Http3),
            ..Default::default()
        };

        H3ConnectArgs {
            socks_proxy: None,
            socket: SocketArgs::default(),
            target_tls: tls,
            proxy_peer_addrs: None,
            quic_peer_addrs: None,
        }
    }

    pub(crate) async fn resolve_target_address(
        &mut self,
        proc_args: &ProcArgs,
        target: &UpstreamAddr,
    ) -> anyhow::Result<()> {
        if let Some(proxy) = &self.socks_proxy {
            let addrs = proc_args.resolve(proxy.peer()).await?;
            self.proxy_peer_addrs = Some(addrs);
        };
        let addrs = proc_args.resolve(target).await?;
        self.quic_peer_addrs = Some(addrs);
        Ok(())
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

            let mut stream = self.socket.tcp_connect_to(peer).await.context(format!(
                "failed to connect to socks5 proxy {}",
                socks5_proxy.peer()
            ))?;

            let socket = self.socket.udp_std_socket_to(peer)?;

            let local_udp_addr = socket
                .local_addr()
                .map_err(|e| anyhow!("failed to get local addr of udp socket: {e}"))?;
            let peer_udp_addr = g3_socks::v5::client::socks5_udp_associate(
                &mut stream,
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

            let limit = &proc_args.udp_sock_speed_limit;
            let runtime = LimitedTokioRuntime::local_limited(
                Socks5UdpTokioRuntime::new(stream, quic_peer),
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
            let socket = self.socket.udp_std_socket_to(quic_peer)?;
            socket
                .connect(quic_peer)
                .map_err(|e| anyhow!("failed to connect local udp socket to {quic_peer}: {e}"))?;

            let limit = &proc_args.udp_sock_speed_limit;
            let runtime = LimitedTokioRuntime::local_limited(
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
        target: &UpstreamAddr,
        stats: &Arc<HttpRuntimeStats>,
        proc_args: &ProcArgs,
    ) -> anyhow::Result<h3_quinn::Connection> {
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
        let quic_config = QuicClientConfig::try_from(tls_client.driver.as_ref().clone())
            .map_err(|e| anyhow!("invalid quic tls config: {e}"))?;
        let mut client_config = ClientConfig::new(Arc::new(quic_config));
        client_config.transport_config(Arc::new(transport));

        let tls_name = match &self.target_tls.tls_name {
            Some(ServerName::DnsName(domain)) => domain.as_ref().to_string(),
            Some(ServerName::IpAddress(ip)) => IpAddr::from(*ip).to_string(),
            Some(_) => return Err(anyhow!("unsupported tls server name type")),
            None => target.host().to_string(),
        };
        let conn = endpoint
            .connect_with(client_config, quic_peer, &tls_name)
            .map_err(|e| anyhow!("failed to create quic client: {e}"))?
            .await
            .map_err(|e| anyhow!("failed to connect: {e}"))?;
        Ok(h3_quinn::Connection::new(conn))
    }

    pub(crate) async fn new_h3_connection(
        &self,
        target: &UpstreamAddr,
        stats: &Arc<HttpRuntimeStats>,
        proc_args: &ProcArgs,
    ) -> anyhow::Result<SendRequest<OpenStreams, Bytes>> {
        let quic_conn = self.new_quic_connection(target, stats, proc_args).await?;

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

    pub(crate) fn parse_args(&mut self, args: &ArgMatches) -> anyhow::Result<()> {
        if let Some(v) = args.get_one::<String>(HTTP_ARG_PROXY) {
            let url = Url::parse(v).context(format!("invalid {HTTP_ARG_PROXY} value"))?;
            let proxy = Proxy::try_from(&url).map_err(|e| anyhow!("invalid proxy: {e}"))?;
            let Proxy::Socks5(proxy) = proxy else {
                return Err(anyhow!("unsupported proxy {v}"));
            };
            self.socks_proxy = Some(proxy);
        }

        self.socket
            .parse_args(args)
            .context("invalid socket config")?;
        self.target_tls
            .parse_tls_args(args)
            .context("invalid target tls config")?;
        Ok(())
    }
}

impl AppendH3ConnectArgs for Command {
    fn append_h3_connect_args(self) -> Self {
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
        .append_rustls_args()
    }
}
