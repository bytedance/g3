/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};
use tokio::time::Interval;

use super::{
    IcapClientConnection, IcapConnectionEofPoller, IcapConnectionPollRequest, IcapConnector,
    IcapServiceConfig,
};
use crate::options::{IcapOptionsRequest, IcapServiceOptions};

const POOL_CMD_CHANNEL_SIZE: usize = 16;

pub(super) enum IcapServiceClientCommand {
    FetchConnection(oneshot::Sender<(IcapClientConnection, Arc<IcapServiceOptions>)>),
    SaveConnection(IcapClientConnection),
}

enum IcapServicePoolCommand {
    UpdateOptions(IcapServiceOptions),
    SaveConnection(IcapClientConnection),
    CheckConnection,
}

pub(super) struct IcapServicePool {
    config: Arc<IcapServiceConfig>,
    options: Arc<IcapServiceOptions>,
    connector: Arc<IcapConnector>,
    check_interval: Interval,
    client_cmd_receiver: flume::Receiver<IcapServiceClientCommand>,
    pool_cmd_sender: mpsc::Sender<IcapServicePoolCommand>,
    pool_cmd_receiver: mpsc::Receiver<IcapServicePoolCommand>,
    conn_req_sender: flume::Sender<IcapConnectionPollRequest>,
    conn_req_receiver: flume::Receiver<IcapConnectionPollRequest>,
    idle_conn_count: Arc<AtomicUsize>,
}

impl IcapServicePool {
    pub(super) fn new(
        config: Arc<IcapServiceConfig>,
        client_cmd_receiver: flume::Receiver<IcapServiceClientCommand>,
        connector: Arc<IcapConnector>,
    ) -> Self {
        let options = Arc::new(IcapServiceOptions::new_expired(config.method));
        let check_interval = tokio::time::interval(config.connection_pool.check_interval());
        let (pool_cmd_sender, pool_cmd_receiver) = mpsc::channel(POOL_CMD_CHANNEL_SIZE);
        let (conn_req_sender, conn_req_receiver) =
            flume::bounded(config.connection_pool.max_idle_count());
        IcapServicePool {
            config,
            options,
            connector,
            check_interval,
            client_cmd_receiver,
            pool_cmd_sender,
            pool_cmd_receiver,
            conn_req_sender,
            conn_req_receiver,
            idle_conn_count: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn idle_conn_count(&self) -> usize {
        self.idle_conn_count.load(Ordering::Relaxed)
    }

    pub(super) async fn into_running(mut self) {
        loop {
            tokio::select! {
                biased;

                _ = self.check_interval.tick() => {
                    self.handle_pool_cmd(IcapServicePoolCommand::CheckConnection);
                }
                r = self.client_cmd_receiver.recv_async() => {
                    match r {
                        Ok(cmd) => self.handle_client_cmd(cmd),
                        Err(_) => break,
                    }
                }
                r = self.pool_cmd_receiver.recv() => {
                    match r {
                        Some(cmd) => self.handle_pool_cmd(cmd),
                        None => unreachable!(),
                    }
                }
            }
        }
    }

    fn check(&mut self) {
        if self.options.expired() {
            let pool_sender = self.pool_cmd_sender.clone();
            let conn_creator = self.connector.clone();
            let config = self.config.clone();
            tokio::spawn(async move {
                if let Ok(mut conn) = conn_creator.create().await {
                    conn.mark_io_inuse();
                    let req = IcapOptionsRequest::new(config.as_ref());
                    if let Ok(options) = req
                        .get_options(&mut conn, config.icap_max_header_size)
                        .await
                    {
                        if pool_sender
                            .send(IcapServicePoolCommand::UpdateOptions(options))
                            .await
                            .is_ok()
                        {
                            let _ = pool_sender
                                .send(IcapServicePoolCommand::SaveConnection(conn))
                                .await;
                        }
                    }
                }
            });
        }

        let current_idle_count = self.idle_conn_count();
        let min_idle_count = self.config.connection_pool.min_idle_count();
        for _i in current_idle_count..min_idle_count {
            if min_idle_count <= self.idle_conn_count() {
                break;
            }
            let pool_sender = self.pool_cmd_sender.clone();
            let conn_creator = self.connector.clone();
            tokio::spawn(async move {
                if let Ok(conn) = conn_creator.create().await {
                    let _ = pool_sender
                        .send(IcapServicePoolCommand::SaveConnection(conn))
                        .await;
                }
            });
        }
    }

    fn handle_client_cmd(&mut self, cmd: IcapServiceClientCommand) {
        match cmd {
            IcapServiceClientCommand::FetchConnection(sender) => {
                if self.idle_conn_count() > 0 {
                    // there maybe race condition, so we have fallback at 1 client side
                    let req_sender = self.conn_req_sender.clone();
                    let options = self.options.clone();
                    tokio::spawn(async move {
                        let _ = req_sender
                            .send_async(IcapConnectionPollRequest::new(sender, options))
                            .await;
                    });
                } else {
                    let conn_creator = self.connector.clone();
                    let options = self.options.clone();
                    tokio::spawn(async move {
                        if let Ok(conn) = conn_creator.create().await {
                            let _ = sender.send((conn, options));
                        }
                    });
                }
            }
            IcapServiceClientCommand::SaveConnection(conn) => {
                if self.idle_conn_count() <= self.config.connection_pool.max_idle_count() {
                    self.save_connection(conn);
                }
            }
        }
    }

    fn handle_pool_cmd(&mut self, cmd: IcapServicePoolCommand) {
        match cmd {
            IcapServicePoolCommand::SaveConnection(conn) => self.save_connection(conn),
            IcapServicePoolCommand::UpdateOptions(options) => self.options = Arc::new(options),
            IcapServicePoolCommand::CheckConnection => self.check(),
        }
    }

    fn save_connection(&mut self, conn: IcapClientConnection) {
        let Some(eof_poller) = IcapConnectionEofPoller::new(conn, &self.conn_req_receiver) else {
            return;
        };

        let idle_count = self.idle_conn_count.clone();
        idle_count.fetch_add(1, Ordering::Relaxed);

        let idle_timeout = self.config.connection_pool.idle_timeout();
        tokio::spawn(async move {
            eof_poller.into_running(idle_timeout).await;
            idle_count.fetch_sub(1, Ordering::Relaxed);
        });
    }
}
