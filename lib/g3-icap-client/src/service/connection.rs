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

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Context;
use tokio::io::BufReader;
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio_rustls::TlsConnector;

use g3_io_ext::rustls::{MaybeTlsStreamReadHalf, MaybeTlsStreamWriteHalf};
use g3_io_ext::{AsyncStream, LimitedBufReadExt};
use g3_types::net::{Host, RustlsClientConfig};

use super::IcapServiceConfig;
use crate::IcapServiceOptions;

pub type IcapClientWriter = MaybeTlsStreamWriteHalf<TcpStream>;
pub type IcapClientReader = BufReader<MaybeTlsStreamReadHalf<TcpStream>>;
pub type IcapClientConnection = (IcapClientWriter, IcapClientReader);

pub(super) struct IcapConnector {
    config: Arc<IcapServiceConfig>,
    tls_client: Option<RustlsClientConfig>,
}

impl IcapConnector {
    pub(super) fn new(config: Arc<IcapServiceConfig>) -> anyhow::Result<Self> {
        let tls_client = match &config.tls_client {
            Some(builder) => {
                let client = builder
                    .build()
                    .context("failed to build TLS client config")?;
                Some(client)
            }
            None => None,
        };
        Ok(IcapConnector { config, tls_client })
    }

    async fn select_peer_addr(&self) -> io::Result<SocketAddr> {
        let upstream = &self.config.upstream;
        match upstream.host() {
            Host::Domain(domain) => {
                let mut addrs = tokio::net::lookup_host((domain.as_ref(), upstream.port())).await?;
                addrs
                    .next()
                    .ok_or_else(|| io::Error::other("no resolved socket address"))
            }
            Host::Ip(ip) => Ok(SocketAddr::new(*ip, upstream.port())),
        }
    }

    pub(super) async fn create(&self) -> io::Result<IcapClientConnection> {
        let peer = self.select_peer_addr().await?;
        let socket = g3_socket::tcp::new_socket_to(
            peer.ip(),
            None,
            &self.config.tcp_keepalive,
            &Default::default(),
            true,
        )?;
        let stream = socket.connect(peer).await?;

        if let Some(client) = &self.tls_client {
            let tls_connector = TlsConnector::from(client.driver.clone());
            match tokio::time::timeout(
                client.handshake_timeout,
                tls_connector.connect(self.config.tls_name.clone(), stream),
            )
            .await
            {
                Ok(Ok(tls_stream)) => {
                    let (r, w) = tls_stream.into_split();
                    Ok((
                        MaybeTlsStreamWriteHalf::Tls(w),
                        BufReader::new(MaybeTlsStreamReadHalf::Tls(r)),
                    ))
                }
                Ok(Err(e)) => Err(e),
                Err(_) => Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "tls handshake with ICAP server timed out",
                )),
            }
        } else {
            let (r, w) = stream.into_split();
            Ok((
                MaybeTlsStreamWriteHalf::Plain(w),
                BufReader::new(MaybeTlsStreamReadHalf::Plain(r)),
            ))
        }
    }
}

pub(super) struct IcapConnectionPollRequest {
    client_sender: oneshot::Sender<(IcapClientConnection, Arc<IcapServiceOptions>)>,
    options: Arc<IcapServiceOptions>,
}

impl IcapConnectionPollRequest {
    pub(super) fn new(
        client_sender: oneshot::Sender<(IcapClientConnection, Arc<IcapServiceOptions>)>,
        options: Arc<IcapServiceOptions>,
    ) -> Self {
        IcapConnectionPollRequest {
            client_sender,
            options,
        }
    }
}

pub(super) struct IcapConnectionEofPoller {
    conn: IcapClientConnection,
    req_receiver: flume::Receiver<IcapConnectionPollRequest>,
}

impl IcapConnectionEofPoller {
    pub(super) fn new(
        conn: IcapClientConnection,
        req_receiver: flume::Receiver<IcapConnectionPollRequest>,
    ) -> Self {
        IcapConnectionEofPoller { conn, req_receiver }
    }

    pub(super) async fn into_running(mut self) {
        tokio::select! {
            _ = self.conn.1.fill_wait_eof() => {}
            r = self.req_receiver.recv_async() => {
                if let Ok(req) = r {
                    let IcapConnectionPollRequest {
                        client_sender,
                        options,
                    } = req;
                    let _ = client_sender.send((self.conn, options));
                }
            }
        }
    }
}
