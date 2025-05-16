/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
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
