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

use tokio::io::BufReader;
use tokio::net::tcp;
use tokio::sync::oneshot;

use crate::IcapServiceOptions;
use g3_io_ext::LimitedBufReadExt;
use g3_types::net::Host;

use super::IcapServiceConfig;

pub type IcapClientWriter = tcp::OwnedWriteHalf;
pub type IcapClientReader = BufReader<tcp::OwnedReadHalf>;
pub type IcapClientConnection = (IcapClientWriter, IcapClientReader);

pub(super) struct IcapConnectionCreator {
    config: Arc<IcapServiceConfig>,
}

impl IcapConnectionCreator {
    pub(super) fn new(config: Arc<IcapServiceConfig>) -> Self {
        IcapConnectionCreator { config }
    }

    async fn select_peer_addr(&self) -> Result<SocketAddr, io::Error> {
        let upstream = &self.config.upstream;
        match upstream.host() {
            Host::Domain(domain) => {
                let mut addrs = tokio::net::lookup_host((domain.as_str(), upstream.port())).await?;
                addrs.next().ok_or_else(|| {
                    io::Error::new(io::ErrorKind::Other, "no resolved socket address")
                })
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
        let (r, w) = stream.into_split();
        Ok((w, BufReader::new(r)))
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
