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

use std::io;
use std::net::SocketAddr;
use std::sync::Arc;

use log::warn;
use tokio::net::UdpSocket;

use g3_geoip::IpLocationBuilder;
use g3_ip_locate::{Request, Response};

use super::FrontendStats;

pub(crate) struct UdpDgramFrontend {
    socket: UdpSocket,
    stats: Arc<FrontendStats>,
}

impl UdpDgramFrontend {
    pub(crate) async fn new(addr: SocketAddr, stats: Arc<FrontendStats>) -> io::Result<Self> {
        let socket = UdpSocket::bind(addr).await?;
        Ok(UdpDgramFrontend { socket, stats })
    }

    pub(crate) async fn recv_req(&self, buf: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.socket.recv_from(buf).await
    }

    pub(crate) async fn send_rsp(&self, data: &[u8], peer: SocketAddr) -> io::Result<()> {
        let nw = self.socket.send_to(data, peer).await?;
        if nw != data.len() {
            Err(io::Error::other(format!(
                "not all data written, only {nw}/{}",
                data.len()
            )))
        } else {
            Ok(())
        }
    }

    pub(crate) async fn into_running(self) {
        let mut recv_buf = [0u8; 1024];

        loop {
            match self.recv_req(&mut recv_buf).await {
                Ok((len, addr)) => {
                    self.stats.add_request_total();

                    let req = match Request::parse_req(&recv_buf[..len]) {
                        Ok(req) => req,
                        Err(e) => {
                            self.stats.add_request_invalid();
                            warn!("invalid request: {e:?}");
                            continue;
                        }
                    };
                    let Some(ip) = req.ip() else {
                        self.stats.add_request_invalid();
                        continue;
                    };

                    let mut builder = IpLocationBuilder::default();

                    if let Some(db) = g3_geoip::store::load_country() {
                        if let Some((net, v)) = db.longest_match(ip) {
                            builder.set_network(net);
                            builder.set_country(v.country);
                            builder.set_continent(v.continent);
                        }
                    }

                    if let Some(asn_db) = g3_geoip::store::load_asn() {
                        if let Some((net, v)) = asn_db.longest_match(ip) {
                            builder.set_network(net);
                            builder.set_as_number(v.number);
                            if let Some(name) = v.isp_name() {
                                builder.set_isp_name(name.to_string());
                            }
                            if let Some(domain) = v.isp_domain() {
                                builder.set_isp_domain(domain.to_string());
                            }
                        }
                    }

                    let Ok(location) = builder.build() else {
                        continue;
                    };

                    match Response::encode_new(ip, location, 300) {
                        Ok(buf) => {
                            self.stats.add_response_total();
                            if self.send_rsp(&buf, addr).await.is_err() {
                                self.stats.add_response_fail();
                            }
                        }
                        Err(e) => {
                            warn!("failed to encode response for ip {ip}: {e}")
                        }
                    }
                }
                Err(e) => {
                    warn!("failed to recv req: {e}")
                }
            }
        }
    }
}
