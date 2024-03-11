/*
 * Copyright 2024 ByteDance and/or its affiliates.
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
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::anyhow;
use async_recursion::async_recursion;
use hickory_client::client::{AsyncClient, ClientHandle};
use hickory_proto::iocompat::AsyncIoTokioAsStd;
use hickory_proto::rr::{DNSClass, Name, RData, RecordType};
use rustls::{ClientConfig, ServerName};
use tokio::net::{TcpStream, UdpSocket};
use tokio::sync::mpsc;

use g3_types::net::{DnsEncryptionConfig, DnsEncryptionProtocol};

use crate::{ResolveDriverError, ResolveError, ResolvedRecord};

#[derive(Clone)]
pub(super) struct DnsRequest {
    domain: Arc<str>,
    rtype: RecordType,
}

impl DnsRequest {
    pub(super) fn query_ipv6(domain: Arc<str>) -> Self {
        DnsRequest {
            domain,
            rtype: RecordType::AAAA,
        }
    }

    pub(super) fn query_ipv4(domain: Arc<str>) -> Self {
        DnsRequest {
            domain,
            rtype: RecordType::A,
        }
    }
}

#[derive(Default)]
struct HickoryClientState {
    failed_count: AtomicUsize,
}

impl HickoryClientState {
    fn add_failed(&self) {
        self.failed_count.fetch_add(1, Ordering::Relaxed);
    }

    fn clear_failed(&self) -> usize {
        self.failed_count.swap(0, Ordering::Relaxed)
    }
}

#[derive(Clone)]
pub(super) struct HickoryClient {
    config: Arc<HickoryClientConfig>,
    state: Arc<HickoryClientState>,
    client: AsyncClient,
}

impl HickoryClient {
    pub(super) async fn new(config: HickoryClientConfig) -> anyhow::Result<Self> {
        let client = config.build_async_client().await?;
        Ok(HickoryClient {
            config: Arc::new(config),
            state: Arc::new(HickoryClientState::default()),
            client,
        })
    }

    pub(super) async fn run(
        mut self,
        req_receiver: flume::Receiver<(DnsRequest, mpsc::Sender<ResolvedRecord>)>,
    ) {
        let (client_sender, mut client_receiver) = mpsc::channel(1);
        let mut check_interval = tokio::time::interval(Duration::from_secs(60));
        loop {
            tokio::select! {
                biased;

                r = req_receiver.recv_async() => {
                    let Ok((req, rsp_sender)) = r else {
                        break;
                    };
                    let client_job = HickoryClientJob {
                        config: self.config.clone(),
                        state: self.state.clone(),
                        try_failed: self.config.each_tries,
                        try_truncated: self.config.retry_tcp(),
                    };
                    let async_client = self.client.clone();
                    tokio::spawn(async move {
                        let r = client_job.run(async_client, req).await;
                        let _ = rsp_sender.send(r).await;
                    });
                }
                _ = check_interval.tick() => {
                    if self.state.clear_failed() > 0 {
                        let client_sender = client_sender.clone();
                        let client_config = self.config.clone();
                        tokio::spawn(async move {
                            if let Ok(client) = client_config.build_async_client().await {
                                let _ = client_sender.try_send(client);
                            }
                        });
                    }
                }
                r = client_receiver.recv() => {
                    if let Some(client) = r {
                        self.client = client;
                    }
                }
            }
        }
    }
}

#[derive(Clone)]
pub(super) struct HickoryClientJob {
    config: Arc<HickoryClientConfig>,
    state: Arc<HickoryClientState>,
    try_failed: i32,
    try_truncated: bool,
}

impl HickoryClientJob {
    #[async_recursion]
    async fn run(mut self, mut async_client: AsyncClient, req: DnsRequest) -> ResolvedRecord {
        let Ok(mut name) = Name::from_ascii(&req.domain) else {
            return ResolvedRecord::failed(
                req.domain,
                self.config.negative_ttl,
                ResolveDriverError::BadName.into(),
            );
        };

        loop {
            match async_client
                .query(name.clone(), DNSClass::IN, req.rtype)
                .await
            {
                Ok(rsp) => {
                    let (mut msg, _) = rsp.into_parts();

                    let response_code = msg.response_code();
                    if let Some(e) = ResolveError::from_response_code(response_code) {
                        return ResolvedRecord::failed(req.domain, self.config.negative_ttl, e);
                    }

                    if msg.truncated() && self.try_truncated {
                        self.try_truncated = false;
                        if let Ok(client) = self.config.new_dns_over_tcp_client().await {
                            return self.run(client, req).await;
                        }
                    }

                    let mut has_cname = false;
                    let mut ips = Vec::with_capacity(4);
                    let mut ttl = 0;
                    for r in msg.take_answers() {
                        ttl = r.ttl();
                        let Some(rdata) = r.data() else {
                            continue;
                        };
                        match rdata {
                            RData::A(v) => {
                                if req.rtype == RecordType::A {
                                    ips.push(IpAddr::V4(v.0));
                                }
                            }
                            RData::AAAA(v) => {
                                if req.rtype == RecordType::AAAA {
                                    ips.push(IpAddr::V6(v.0))
                                }
                            }
                            RData::CNAME(v) => {
                                if name.eq(r.name()) {
                                    has_cname = true;
                                    name = v.0.clone();
                                }
                            }
                            _ => {}
                        }
                    }
                    return if ips.is_empty() {
                        if has_cname {
                            self.try_truncated = true;
                            continue;
                        }
                        ResolvedRecord::resolved(req.domain, self.config.negative_ttl, ips)
                    } else {
                        let ttl =
                            ttl.clamp(self.config.positive_min_ttl, self.config.positive_max_ttl);
                        ResolvedRecord::resolved(req.domain, ttl, ips)
                    };
                }
                Err(e) => {
                    self.state.add_failed();
                    self.try_failed -= 1;
                    if self.try_failed > 0 {
                        if let Ok(client) = self.config.build_async_client().await {
                            return self.run(client, req).await;
                        }
                    }
                    return ResolvedRecord::failed(req.domain, self.config.negative_ttl, e.into());
                }
            }
        }
    }
}

#[derive(Clone)]
pub(super) struct HickoryClientConfig {
    pub(super) target: SocketAddr,
    pub(super) bind: Option<SocketAddr>,
    pub(super) encryption: Option<DnsEncryptionConfig>,
    pub(super) connect_timeout: Duration,
    pub(super) request_timeout: Duration,
    pub(super) each_tries: i32,
    pub(super) positive_min_ttl: u32,
    pub(super) positive_max_ttl: u32,
    pub(super) negative_ttl: u32,
}

impl HickoryClientConfig {
    fn retry_tcp(&self) -> bool {
        self.encryption.is_none()
    }

    async fn build_async_client(&self) -> anyhow::Result<AsyncClient> {
        if let Some(ec) = &self.encryption {
            let tls_client = ec.tls_client().driver.as_ref().clone();

            match ec.protocol() {
                DnsEncryptionProtocol::Tls => {
                    self.new_dns_over_tls_client(tls_client, ec.tls_name().clone())
                        .await
                }
                DnsEncryptionProtocol::Https => {
                    self.new_dns_over_h2_client(tls_client, ec.tls_name().clone())
                        .await
                }
                #[cfg(feature = "quic")]
                DnsEncryptionProtocol::Quic => {
                    self.new_dns_over_quic_client(tls_client, ec.tls_name())
                        .await
                }
                #[cfg(feature = "quic")]
                DnsEncryptionProtocol::H3 => {
                    self.new_dns_over_h3_client(tls_client, ec.tls_name()).await
                }
            }
        } else {
            self.new_dns_over_udp_client().await
        }
    }

    async fn new_dns_over_udp_client(&self) -> anyhow::Result<AsyncClient> {
        // random port is used here
        let client_connect =
            hickory_client::udp::UdpClientStream::<UdpSocket>::with_bind_addr_and_timeout(
                self.target,
                self.bind,
                self.request_timeout,
            );

        let (client, bg) = AsyncClient::connect(client_connect)
            .await
            .map_err(|e| anyhow!("failed to create udp async client: {e}"))?;
        tokio::spawn(bg);
        Ok(client)
    }

    async fn new_dns_over_tcp_client(&self) -> anyhow::Result<AsyncClient> {
        let (stream, sender) =
            hickory_client::tcp::TcpClientStream::<AsyncIoTokioAsStd<TcpStream>>::with_bind_addr_and_timeout(
                self.target,
                self.bind,
                self.connect_timeout,
            );

        let (client, bg) = AsyncClient::with_timeout(stream, sender, self.request_timeout, None)
            .await
            .map_err(|e| anyhow!("failed to create tcp async client: {e}"))?;
        tokio::spawn(bg);
        Ok(client)
    }

    async fn new_dns_over_tls_client(
        &self,
        tls_client: ClientConfig,
        tls_name: ServerName,
    ) -> anyhow::Result<AsyncClient> {
        use hickory_proto::BufDnsStreamHandle;

        let (message_sender, outbound_messages) = BufDnsStreamHandle::new(self.target);

        let tls_connect = g3_hickory_client::io::tls::connect(
            self.target,
            self.bind,
            tls_client,
            tls_name,
            outbound_messages,
            self.connect_timeout,
        );

        let (client, bg) = AsyncClient::with_timeout(
            Box::pin(tls_connect),
            message_sender,
            self.request_timeout,
            None,
        )
        .await
        .map_err(|e| anyhow!("failed to create tls async client: {e}"))?;
        tokio::spawn(bg);
        Ok(client)
    }

    async fn new_dns_over_h2_client(
        &self,
        tls_client: ClientConfig,
        tls_name: ServerName,
    ) -> anyhow::Result<AsyncClient> {
        let client_connect = g3_hickory_client::io::h2::connect(
            self.target,
            self.bind,
            tls_client,
            tls_name,
            self.connect_timeout,
            self.request_timeout,
        );

        let (client, bg) = AsyncClient::connect(Box::pin(client_connect))
            .await
            .map_err(|e| anyhow!("failed to create h2 async client: {e}"))?;
        tokio::spawn(bg);
        Ok(client)
    }

    #[cfg(feature = "quic")]
    async fn new_dns_over_quic_client(
        &self,
        tls_client: ClientConfig,
        tls_name: &ServerName,
    ) -> anyhow::Result<AsyncClient> {
        let tls_name = match tls_name {
            ServerName::DnsName(domain) => domain.as_ref().to_string(),
            ServerName::IpAddress(ip) => ip.to_string(),
            _ => return Err(anyhow!("unsupported tls server name type")),
        };

        let client_connect = g3_hickory_client::io::quic::connect(
            self.target,
            self.bind,
            tls_client,
            tls_name,
            self.connect_timeout,
            self.request_timeout,
        );

        let (client, bg) = AsyncClient::connect(Box::pin(client_connect))
            .await
            .map_err(|e| anyhow!("failed to create udp async client: {e}"))?;
        tokio::spawn(bg);
        Ok(client)
    }

    #[cfg(feature = "quic")]
    async fn new_dns_over_h3_client(
        &self,
        tls_client: ClientConfig,
        tls_name: &ServerName,
    ) -> anyhow::Result<AsyncClient> {
        let tls_name = match tls_name {
            ServerName::DnsName(domain) => domain.as_ref().to_string(),
            ServerName::IpAddress(ip) => ip.to_string(),
            _ => return Err(anyhow!("unsupported tls server name type")),
        };

        let client_connect = g3_hickory_client::io::h3::connect(
            self.target,
            self.bind,
            tls_client,
            tls_name,
            self.connect_timeout,
            self.request_timeout,
        );

        let (client, bg) = AsyncClient::connect(Box::pin(client_connect))
            .await
            .map_err(|e| anyhow!("failed to create h3 async client: {e}"))?;
        tokio::spawn(bg);
        Ok(client)
    }
}
