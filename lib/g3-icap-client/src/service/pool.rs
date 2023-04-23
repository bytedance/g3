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

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use tokio::sync::{mpsc, oneshot};
use tokio::time::Interval;

use super::{
    IcapClientConnection, IcapConnectionCreator, IcapConnectionEofPoller,
    IcapConnectionPollRequest, IcapServiceConfig,
};
use crate::options::{IcapOptionsRequest, IcapServiceOptions};

pub(super) enum IcapServiceClientCommand {
    FetchConnection(oneshot::Sender<(IcapClientConnection, Arc<IcapServiceOptions>)>),
    SaveConnection(IcapClientConnection),
}

enum IcapServicePoolCommand {
    UpdateOptions(IcapServiceOptions),
    SaveConnection(IcapClientConnection),
}

pub(super) struct IcapServicePool {
    config: Arc<IcapServiceConfig>,
    options: Arc<IcapServiceOptions>,
    conn_creator: Arc<IcapConnectionCreator>,
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
        conn_creator: Arc<IcapConnectionCreator>,
    ) -> Self {
        let options = Arc::new(IcapServiceOptions::new_expired(config.method));
        let check_interval = tokio::time::interval(config.connection_pool.check_interval);
        let (pool_cmd_sender, pool_cmd_receiver) =
            mpsc::channel(config.connection_pool.max_idle_count);
        let (conn_req_sender, conn_req_receiver) =
            flume::bounded(config.connection_pool.max_idle_count);
        IcapServicePool {
            config,
            options,
            conn_creator,
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
                    self.check();
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
            let conn_creator = self.conn_creator.clone();
            let config = self.config.clone();
            tokio::spawn(async move {
                if let Ok(mut conn) = conn_creator.create().await {
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
        if current_idle_count < self.config.connection_pool.min_idle_count {
            for _i in current_idle_count..self.config.connection_pool.min_idle_count {
                let pool_sender = self.pool_cmd_sender.clone();
                let conn_creator = self.conn_creator.clone();
                tokio::spawn(async move {
                    if let Ok(conn) = conn_creator.create().await {
                        let _ = pool_sender
                            .send(IcapServicePoolCommand::SaveConnection(conn))
                            .await;
                    }
                });
            }
        }
    }

    fn handle_client_cmd(&mut self, cmd: IcapServiceClientCommand) {
        match cmd {
            IcapServiceClientCommand::FetchConnection(sender) => {
                if self.idle_conn_count() > 0 {
                    // there maybe race condition, so we have fallback at client side
                    let req_sender = self.conn_req_sender.clone();
                    let options = self.options.clone();
                    tokio::spawn(async move {
                        let _ = req_sender
                            .send_async(IcapConnectionPollRequest::new(sender, options))
                            .await;
                    });
                } else {
                    let conn_creator = self.conn_creator.clone();
                    let options = self.options.clone();
                    tokio::spawn(async move {
                        if let Ok(conn) = conn_creator.create().await {
                            let _ = sender.send((conn, options));
                        }
                    });
                }
            }
            IcapServiceClientCommand::SaveConnection(conn) => self.save_connection(conn),
        }
    }

    fn handle_pool_cmd(&mut self, cmd: IcapServicePoolCommand) {
        match cmd {
            IcapServicePoolCommand::SaveConnection(conn) => self.save_connection(conn),
            IcapServicePoolCommand::UpdateOptions(options) => self.options = Arc::new(options),
        }
    }

    fn save_connection(&mut self, conn: IcapClientConnection) {
        // it's ok to skip compare_swap as we only increase the idle count in the same future context
        if self.idle_conn_count() < self.config.connection_pool.max_idle_count {
            let idle_count = self.idle_conn_count.clone();
            // relaxed is fine as we only increase it here in the same future context
            idle_count.fetch_add(1, Ordering::Relaxed);
            let eof_poller = IcapConnectionEofPoller::new(conn, self.conn_req_receiver.clone());
            tokio::spawn(async move {
                eof_poller.into_running().await;
                idle_count.fetch_sub(1, Ordering::Relaxed);
            });
        }
    }
}
