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

use std::net::IpAddr;
use std::sync::Arc;

use log::warn;
use tokio::sync::broadcast;

use g3_geoip_types::{IpLocation, IpLocationBuilder};
use g3_ip_locate::{Request, Response};
use g3_types::net::UdpListenConfig;

mod stats;
pub(crate) use stats::FrontendStats;

mod udp_dgram;
use udp_dgram::UdpDgramFrontend;

pub(super) struct Frontend {
    io: UdpDgramFrontend,
    stats: Arc<FrontendStats>,
}

impl Frontend {
    pub(super) fn new(
        listen_config: &UdpListenConfig,
        stats: Arc<FrontendStats>,
    ) -> anyhow::Result<Self> {
        let io = UdpDgramFrontend::new(listen_config)?;
        Ok(Frontend { io, stats })
    }

    pub(super) async fn run(
        self,
        mut quit_receiver: broadcast::Receiver<()>,
    ) -> anyhow::Result<()> {
        let mut recv_buf = [0u8; 1024];

        loop {
            tokio::select! {
                biased;

                r = self.io.recv_req(&mut recv_buf) => {
                    match r {
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

                            let Some(location) = self.fetch(ip) else {
                                continue;
                            };

                            match Response::encode_new(ip, location, 300) {
                                Ok(buf) => {
                                    self.stats.add_response_total();
                                    if self.io.send_rsp(&buf, addr).await.is_err() {
                                        self.stats.add_response_fail();
                                    }
                                }
                                Err(e) => {
                                    warn!("failed to encode response for ip {ip}: {e}");
                                }
                            }
                        }
                        Err(e) => {
                            warn!("failed to recv req: {e}");
                        }
                    }
                }
                _ = quit_receiver.recv() => return Ok(()),
            }
        }
    }

    fn fetch(&self, ip: IpAddr) -> Option<IpLocation> {
        let mut builder = IpLocationBuilder::default();

        if let Some(db) = g3_geoip_db::store::load_country() {
            if let Some((net, v)) = db.longest_match(ip) {
                builder.set_network(net);
                builder.set_country(v.country);
                builder.set_continent(v.continent);
            }
        }

        if let Some(asn_db) = g3_geoip_db::store::load_asn() {
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

        builder.build().ok()
    }
}
