/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2025 ByteDance and/or its affiliates.
 */

use anyhow::anyhow;
use log::{info, warn};
use std::path::PathBuf;
use tokio::net::UnixDatagram;
use tokio::net::unix::SocketAddr as UnixSocketAddr;
use tokio::sync::broadcast;

use crate::server::{BaseServer, ReloadServer, ServerReloadCommand};

pub trait ReceiveUnixDatagramServer: BaseServer {
    fn receive_unix_packet(&self, packet: &[u8], peer_addr: UnixSocketAddr);
}

#[derive(Clone)]
pub struct ReceiveUnixDatagramRuntime<S> {
    server: S,
    server_type: &'static str,
    server_version: usize,
    listen_path: PathBuf,
    //listen_stats: Arc<ListenStats>,
}

impl<S> ReceiveUnixDatagramRuntime<S>
where
    S: ReceiveUnixDatagramServer + ReloadServer + Clone + Send + Sync + 'static,
{
    pub fn new(server: S, listen_path: PathBuf) -> Self {
        let server_type = server.r#type();
        let server_version = server.version();
        ReceiveUnixDatagramRuntime {
            server,
            server_type,
            server_version,
            listen_path,
        }
    }

    fn pre_start(&self) {
        info!(
            "started {} SRT[{}_v{}]",
            self.server_type,
            self.server.name(),
            self.server_version,
        );
        //self.listen_stats.add_running_runtime();
    }

    fn pre_stop(&self) {
        info!(
            "stopping {} SRT[{}_v{}]",
            self.server_type,
            self.server.name(),
            self.server_version,
        );
    }

    fn post_stop(&self) {
        info!(
            "stopped {} SRT[{}_v{}]",
            self.server_type,
            self.server.name(),
            self.server_version,
        );
        //self.listen_stats.del_running_runtime();
    }

    async fn run(
        mut self,
        socket: UnixDatagram,
        mut server_reload_channel: broadcast::Receiver<ServerReloadCommand>,
    ) {
        use broadcast::error::RecvError;

        let mut buf = [0u8; u16::MAX as usize];
        loop {
            tokio::select! {
                biased;

                ev = server_reload_channel.recv() => {
                    match ev {
                        Ok(ServerReloadCommand::ReloadVersion(version)) => {
                            info!("SRT[{}_v{}] received reload request from v{version}",
                                self.server.name(), self.server_version);
                            let new_server = self.server.reload();
                            self.server_version = new_server.version();
                            self.server = new_server;
                            continue;
                        }
                        Ok(ServerReloadCommand::QuitRuntime) => {},
                        Err(RecvError::Closed) => {},
                        Err(RecvError::Lagged(dropped)) => {
                            warn!("SRT[{}_v{}] server {} reload notify channel overflowed, {dropped} msg dropped",
                                self.server.name(), self.server_version, self.server.name());
                            continue;
                        }
                    }

                    info!("SRT[{}_v{}] will go offline",
                        self.server.name(), self.server_version);
                    self.pre_stop();
                    break;
                }
                r = socket.recv_from(&mut buf) => {
                    match r {
                        Ok((len, peer_addr)) => {
                            // TODO add stats
                            self.server.receive_unix_packet(&buf[..len], peer_addr);
                        }
                        Err(e) => {
                            warn!("SRT[{}_v{}] error receiving data from socket, error: {e}",
                                self.server.name(), self.server_version);
                        }
                    }
                }
            }
        }

        self.post_stop();
    }

    pub fn spawn(
        self,
        server_reload_sender: &broadcast::Sender<ServerReloadCommand>,
    ) -> anyhow::Result<()> {
        if self.listen_path.exists() {
            std::fs::remove_file(&self.listen_path).map_err(|e| {
                anyhow!(
                    "failed to delete existed socket file {}: {e}",
                    self.listen_path.display()
                )
            })?;
        }
        let socket = UnixDatagram::bind(&self.listen_path).map_err(|e| {
            anyhow!(
                "failed to create unix datagram socket on path {}: {e}",
                self.listen_path.display()
            )
        })?;
        let server_reload_channel = server_reload_sender.subscribe();
        tokio::spawn(async move {
            self.pre_start();
            self.run(socket, server_reload_channel).await;
        });
        Ok(())
    }
}
