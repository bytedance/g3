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

use std::net::SocketAddr;
use std::sync::Arc;

use ::log::warn;
use anyhow::{anyhow, Context};
use tokio::runtime::Handle;
use tokio::time::Instant;

use g3_histogram::{HistogramStats, RotatingHistogram};

pub mod config;

mod build;

pub mod opts;
use opts::ProcArgs;

mod stat;

mod backend;
use backend::{BackendStats, OpensslBackend};

mod frontend;
use frontend::{FrontendStats, ResponseData, UdpDgramFrontend};
use g3_types::ext::DurationExt;

struct BackendRequest {
    host: String,
    peer: SocketAddr,
    recv_time: Instant,
}

struct BackendResponse {
    data: ResponseData,
    peer: SocketAddr,
    recv_time: Instant,
}

impl BackendRequest {
    fn response(&self, data: ResponseData) -> BackendResponse {
        BackendResponse {
            data,
            peer: self.peer,
            recv_time: self.recv_time,
        }
    }
}

impl BackendResponse {
    fn duration(&self) -> u64 {
        self.recv_time.elapsed().as_nanos_u64()
    }
}

pub async fn run(proc_args: &ProcArgs) -> anyhow::Result<()> {
    let (req_sender, req_receiver) = flume::bounded::<BackendRequest>(1024);
    let (rsp_sender, rsp_receiver) = flume::bounded::<BackendResponse>(1024);

    let backend_config =
        config::get_backend_config().ok_or_else(|| anyhow!("no backend config available"))?;
    let backend_stats = Arc::new(BackendStats::default());

    let duration_stats = if backend_config.request_duration_quantile.is_empty() {
        Arc::new(HistogramStats::new())
    } else {
        Arc::new(HistogramStats::with_quantiles(
            &backend_config.request_duration_quantile,
        ))
    };
    let (histogram, recorder) =
        RotatingHistogram::<u64>::new(backend_config.request_duration_rotate);
    histogram.spawn_refresh(duration_stats.clone());

    let workers = g3_daemon::runtime::worker::foreach(|h| {
        let backend = OpensslBackend::new(&backend_config, &backend_stats)
            .context(format!("failed to build backend for worker {}", h.id))?;
        backend.spawn(&h.handle, h.id, req_receiver.clone(), rsp_sender.clone());
        Ok::<(), anyhow::Error>(())
    })?;
    if workers < 1 {
        let backend = OpensslBackend::new(&backend_config, &backend_stats)
            .context("failed to build backend for main runtime")?;
        backend.spawn(&Handle::current(), 0, req_receiver, rsp_sender);
    }

    let frontend_stats = Arc::new(FrontendStats::default());
    if let Some(stats_config) = g3_daemon::stat::config::get_global_stat_config() {
        stat::spawn_working_thread(
            stats_config,
            backend_stats,
            duration_stats,
            frontend_stats.clone(),
        )?;
    }

    if let Some(addr) = proc_args.udp_addr {
        let frontend = UdpDgramFrontend::new(addr).await?;

        let mut rcv_buf = [0u8; 1024];

        loop {
            tokio::select! {
                r = frontend.recv_req(&mut rcv_buf) => {
                    frontend_stats.add_request_total();
                    let recv_time = Instant::now();
                    match r {
                        Ok((len, peer)) => match crate::frontend::decode_req(&rcv_buf[0..len]) {
                            Ok(host) => {
                                let req = BackendRequest {host, peer, recv_time};
                                if let Err(e) = req_sender.send_async(req).await {
                                    return Err(anyhow!("failed to send request to backend: {e}"));
                                }
                            }
                            Err(e) => {
                                frontend_stats.add_request_invalid();
                                warn!("invalid request from peer {peer}: {e:?}");
                            }
                        }
                        Err(e) => return Err(anyhow!("frontend recv error: {e:?}")),
                    }
                }
                r = rsp_receiver.recv_async() => {
                    match r {
                        Ok(rsp) => match rsp.data.encode() {
                            Ok(buf) => {
                                frontend_stats.add_response_total();
                                match frontend.send_rsp(buf.as_slice(), rsp.peer).await {
                                    Ok(_) => {
                                        let _ = recorder.record(rsp.duration());
                                    }
                                    Err(e) => {
                                        frontend_stats.add_response_fail();
                                        warn!("write response back error: {e:?}");
                                    }
                                }
                            }
                            Err(e) => return Err(anyhow!("response encode error: {e:?}")),
                        }
                        Err(e) => return Err(anyhow!("recv from backend failed: {e}")),
                    }
                }
            }
        }
    } else {
        Err(anyhow!("no frontend found"))
    }
}
