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

use anyhow::{anyhow, Context};
use log::{debug, warn};
use tokio::runtime::Handle;
use tokio::time::Instant;

use g3_cert_agent::Request;
use g3_types::ext::DurationExt;

pub mod config;

mod build;

pub mod opts;
use opts::ProcArgs;

mod stat;

mod backend;
use backend::{BackendStats, OpensslBackend};

mod frontend;
use frontend::{FrontendStats, GeneratedData, UdpDgramFrontend};

struct BackendRequest {
    user_req: Request,
    peer: SocketAddr,
    recv_time: Instant,
}

struct BackendResponse {
    user_req: Request,
    generated: GeneratedData,
    peer: SocketAddr,
    recv_time: Instant,
}

impl BackendRequest {
    fn into_response(self, generated: GeneratedData) -> BackendResponse {
        BackendResponse {
            user_req: self.user_req,
            generated,
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

    let (duration_recorder, duration_stats) = backend_config.duration_stats.build_spawned(None);

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

    let udp_listen_addr = proc_args.udp_listen_addr();
    let frontend = UdpDgramFrontend::new(udp_listen_addr).await?;

    let mut rcv_buf = [0u8; 16384];
    loop {
        tokio::select! {
            r = frontend.recv_req(&mut rcv_buf) => {
                frontend_stats.add_request_total();
                let recv_time = Instant::now();
                match r {
                    Ok((len, peer)) => match Request::parse_req(&rcv_buf[0..len]) {
                        Ok(user_req) => {
                            debug!("{} - request received", user_req.host());
                            let req = BackendRequest {user_req, peer, recv_time};
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
                    Ok(rsp) =>{
                        match rsp.user_req.encode_rsp(&rsp.generated.cert, &rsp.generated.key, rsp.generated.ttl) {
                            Ok(buf) => {
                                frontend_stats.add_response_total();
                                match frontend.send_rsp(buf.as_slice(), rsp.peer).await {
                                    Ok(_) => {
                                        let duration_nanos = rsp.duration();
                                        debug!("{} - duration: {}ns, rsp size: {}", rsp.user_req.host(), duration_nanos, buf.len());
                                        let _ = duration_recorder.record(duration_nanos);
                                    }
                                    Err(e) => {
                                        frontend_stats.add_response_fail();
                                        warn!("write response back error: {e:?}");
                                    }
                                }
                            }
                            Err(e) => return Err(anyhow!("response encode error: {e:?}")),
                        }
                    }
                    Err(e) => return Err(anyhow!("recv from backend failed: {e}")),
                }
            }
        }
    }
}
