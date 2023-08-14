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
use std::time::Duration;

use ::log::{debug, error, warn};
use anyhow::{anyhow, Context};

pub mod config;

mod build;

pub mod opts;
use opts::ProcArgs;

mod backend;
use backend::OpensslBackend;

mod frontend;
use frontend::{ResponseData, UdpDgramFrontend};

pub async fn run(proc_args: &ProcArgs) -> anyhow::Result<()> {
    let (req_sender, req_receiver) = flume::bounded::<(String, SocketAddr)>(1024);
    let (rsp_sender, rsp_receiver) = flume::bounded::<(ResponseData, SocketAddr)>(1024);

    let backend_config =
        config::get_backend_config().ok_or_else(|| anyhow!("no backend config available"))?;

    g3_daemon::runtime::worker::foreach(|h| {
        let id = h.id;
        let mut backend = OpensslBackend::new(&backend_config)
            .context(format!("failed to build backend {id}"))?;
        let req_receiver = req_receiver.clone();
        let rsp_sender = rsp_sender.clone();

        h.handle.spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(300));

            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        if let Err(e) = backend.refresh() {
                            warn!("failed to refresh backend: {e:?}");
                        }
                    }
                    r = req_receiver.recv_async() => {
                        let Ok((host, peer_addr)) = r else {
                            break
                        };

                        match backend.generate(&host) {
                            Ok(data) => {
                                debug!("Worker#{id} got certificate for host {host}");
                                if let Err(e) = rsp_sender.send_async((data, peer_addr)).await {
                                    error!(
                                        "Worker#{id} failed to send certificate for host {host} to frontend: {e}"
                                    );
                                    break;
                                }
                            }
                            Err(e) => {
                                warn!("Worker#{id} generate for {host} failed: {e:?}");
                            }
                        }
                    }
                }
            }
        });
        Ok::<(), anyhow::Error>(())
    })?;

    if let Some(addr) = proc_args.udp_addr {
        let frontend = UdpDgramFrontend::new(addr).await?;

        let mut rcv_buf = [0u8; 1024];

        loop {
            tokio::select! {
                r = frontend.recv_req(&mut rcv_buf) => {
                    match r {
                        Ok((len, peer_addr)) => match crate::frontend::decode_req(&rcv_buf[0..len]) {
                            Ok(host) => {
                                if let Err(e) = req_sender.send_async((host, peer_addr)).await {
                                    return Err(anyhow!("failed to send request to backend: {e}"));
                                }
                            }
                            Err(e) => warn!("FG#0 invalid request from peer {peer_addr}: {e:?}"),
                        }
                        Err(e) => return Err(anyhow!("frontend recv error: {e:?}")),
                    }
                }
                r = rsp_receiver.recv_async() => {
                    match r {
                        Ok((data, peer_addr)) => match data.encode() {
                            Ok(buf) => {
                                if let Err(e) = frontend.send_rsp(buf.as_slice(), peer_addr).await {
                                    warn!("FG#0 write response back error: {e:?}");
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
