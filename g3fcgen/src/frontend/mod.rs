/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use anyhow::anyhow;
use log::{debug, warn};
use tokio::time::Instant;

use g3_cert_agent::Request;
use g3_histogram::HistogramRecorder;
use g3_types::net::UdpListenConfig;

use crate::{BackendRequest, BackendResponse};

mod stats;
pub(crate) use stats::FrontendStats;

mod udp_dgram;
use udp_dgram::UdpDgramIo;

#[derive(Debug)]
pub(crate) struct GeneratedData {
    pub(crate) cert: String,
    pub(crate) key: Vec<u8>,
    pub(crate) ttl: u32,
}

pub(super) struct Frontend {
    io: UdpDgramIo,
    stats: Arc<FrontendStats>,
    duration_recorder: HistogramRecorder<u64>,
    rsp_receiver: kanal::AsyncReceiver<BackendResponse>,
}

impl Frontend {
    pub(super) fn new(
        listen_config: &UdpListenConfig,
        duration_recorder: HistogramRecorder<u64>,
        rsp_receiver: kanal::AsyncReceiver<BackendResponse>,
    ) -> anyhow::Result<Self> {
        let io = UdpDgramIo::new(listen_config)?;
        Ok(Frontend {
            io,
            stats: Arc::new(FrontendStats::default()),
            duration_recorder,
            rsp_receiver,
        })
    }

    pub(super) fn stats(&self) -> Arc<FrontendStats> {
        self.stats.clone()
    }

    pub(super) async fn run(
        self,
        req_sender: kanal::AsyncSender<BackendRequest>,
    ) -> anyhow::Result<()> {
        let mut clt_c = Box::pin(tokio::signal::ctrl_c());

        let mut rcv_buf = [0u8; 16384];
        loop {
            tokio::select! {
                r = self.io.recv_req(&mut rcv_buf) => {
                    self.stats.add_request_total();
                    let recv_time = Instant::now();
                    match r {
                        Ok((len, peer)) => match Request::parse_req(&rcv_buf[0..len]) {
                            Ok(user_req) => {
                                debug!("{} - request received", user_req.host());
                                let req = BackendRequest {user_req, peer, recv_time};
                                if let Err(e) = req_sender.send(req).await {
                                    return Err(anyhow!("failed to send request to backend: {e}"));
                                }
                            }
                            Err(e) => {
                                self.stats.add_request_invalid();
                                warn!("invalid request from peer {peer}: {e:?}");
                            }
                        }
                        Err(e) => return Err(anyhow!("frontend recv error: {e:?}")),
                    }
                }
                r = self.rsp_receiver.recv() => {
                    match r {
                        Ok(rsp) => self.handle_rsp(rsp).await,
                        Err(e) => return Err(anyhow!("recv from backend failed: {e}")),
                    }
                }
                r = &mut clt_c => {
                    if let Err(e) = r {
                        warn!("failed to read Ctrl-C signal: {e}");
                    } else {
                        debug!("received Ctrl-C signal, start shutdown now");
                    }
                    break;
                }
            }
        }

        drop(req_sender);
        while let Ok(rsp) = self.rsp_receiver.recv().await {
            self.handle_rsp(rsp).await;
        }

        debug!("all requests served, quit now");
        Ok(())
    }

    async fn handle_rsp(&self, rsp: BackendResponse) {
        match rsp
            .user_req
            .encode_rsp(&rsp.generated.cert, &rsp.generated.key, rsp.generated.ttl)
        {
            Ok(buf) => {
                self.stats.add_response_total();
                match self.io.send_rsp(buf.as_slice(), rsp.peer).await {
                    Ok(_) => {
                        let duration_nanos = rsp.duration();
                        debug!(
                            "{} - duration: {}ns, rsp size: {}",
                            rsp.user_req.host(),
                            duration_nanos,
                            buf.len()
                        );
                        let _ = self.duration_recorder.record(duration_nanos);
                    }
                    Err(e) => {
                        self.stats.add_response_fail();
                        warn!("write response back error: {e:?}");
                    }
                }
            }
            Err(e) => warn!("response encode error: {e:?}"),
        }
    }
}
