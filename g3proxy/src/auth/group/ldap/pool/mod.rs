/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2026 G3-OSS developers.
 */

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use log::warn;
use tokio::sync::{mpsc, oneshot};

use g3_types::auth::UserAuthError;

use crate::config::auth::LdapUserGroupConfig;

mod connect;
use connect::LdapConnector;

mod task;
use task::LdapAuthTask;

enum PoolCommand {
    Exit,
    NeedMoreConnection,
    ConnectFailed,
    ConnectionClosed,
}

struct LdapAuthRequest {
    username: String,
    password: String,
    retry: bool,
    result_sender: oneshot::Sender<Option<(String, String)>>,
}

pub(super) struct LdapAuthPoolHandle {
    config: Arc<LdapUserGroupConfig>,
    req_sender: kanal::AsyncSender<LdapAuthRequest>,
    cmd_sender: mpsc::Sender<PoolCommand>,
}

impl Drop for LdapAuthPoolHandle {
    fn drop(&mut self) {
        let _ = self.cmd_sender.try_send(PoolCommand::Exit);
    }
}

impl LdapAuthPoolHandle {
    pub(super) async fn check_username_password(
        &self,
        username: &str,
        password: &str,
    ) -> Result<(), UserAuthError> {
        let (sender, receiver) = oneshot::channel();
        let req = LdapAuthRequest {
            username: username.to_string(),
            password: password.to_string(),
            retry: true,
            result_sender: sender,
        };

        if self.req_sender.is_full() {
            let _ = self.cmd_sender.try_send(PoolCommand::NeedMoreConnection);
        }
        let _ = self.req_sender.send(req).await;

        match tokio::time::timeout(self.config.queue_wait_timeout, receiver).await {
            Ok(Ok(Some(_))) => Ok(()),
            Ok(Ok(None)) => Err(UserAuthError::TokenNotMatch),
            Ok(Err(_)) => Err(UserAuthError::RemoteError),
            Err(_) => Err(UserAuthError::RemoteTimeout),
        }
    }
}

pub(super) struct LdapAuthPool {
    config: Arc<LdapUserGroupConfig>,
    connector: Arc<LdapConnector>,
    req_receiver: kanal::AsyncReceiver<LdapAuthRequest>,
    cmd_sender: mpsc::Sender<PoolCommand>,
    cmd_receiver: mpsc::Receiver<PoolCommand>,
    idle_conn_count: Arc<AtomicUsize>,
    expected_idle_count: usize,
}

impl LdapAuthPool {
    pub(super) fn create(config: Arc<LdapUserGroupConfig>) -> anyhow::Result<LdapAuthPoolHandle> {
        let connector = LdapConnector::new(config.clone())?;
        let connector = Arc::new(connector);

        let (req_sender, req_receiver) = kanal::bounded_async(config.queue_channel_size);
        let (cmd_sender, cmd_receiver) = mpsc::channel(config.connection_pool.max_idle_count());

        let pool = LdapAuthPool {
            config: config.clone(),
            connector,
            req_receiver,
            cmd_sender: cmd_sender.clone(),
            cmd_receiver,
            idle_conn_count: Arc::new(AtomicUsize::new(0)),
            expected_idle_count: config.connection_pool.min_idle_count(),
        };
        tokio::spawn(async move { pool.into_running().await });

        Ok(LdapAuthPoolHandle {
            config,
            req_sender,
            cmd_sender,
        })
    }

    async fn into_running(mut self) {
        let mut check_interval =
            tokio::time::interval(self.config.connection_pool.check_interval());

        loop {
            tokio::select! {
                r = self.cmd_receiver.recv() => {
                    match r {
                        Some(PoolCommand::Exit) => {
                            return;
                        }
                        Some(PoolCommand::ConnectFailed) => {}
                        Some(PoolCommand::ConnectionClosed) => {
                            if self.idle_conn_count() < self.expected_idle_count {
                                self.create_connection();
                            }
                        }
                        Some(PoolCommand::NeedMoreConnection) => {
                            if self.expected_idle_count < self.config.connection_pool.max_idle_count() {
                                self.expected_idle_count += 1;
                            }
                            if self.idle_conn_count() < self.config.connection_pool.max_idle_count() {
                                self.create_connection();
                            }
                        }
                        None => {
                            return;
                        }
                    }
                }
                _ = check_interval.tick() => {
                    self.check();
                }
            }
        }
    }

    fn idle_conn_count(&self) -> usize {
        self.idle_conn_count.load(Ordering::Relaxed)
    }

    fn check(&mut self) {
        if self.expected_idle_count > self.config.connection_pool.min_idle_count() {
            let decrease =
                (self.config.connection_pool.min_idle_count() - self.expected_idle_count) / 4;
            if decrease > 0 {
                self.expected_idle_count -= decrease;
            } else {
                self.expected_idle_count -= 1;
            }
        }
        let current_idle_count = self.idle_conn_count();
        if current_idle_count < self.expected_idle_count {
            for _i in current_idle_count..self.expected_idle_count {
                self.create_connection();
            }
        }
    }

    fn create_connection(&self) {
        let task = LdapAuthTask::new(self.config.clone(), self.connector.clone());
        let idle_count = self.idle_conn_count.clone();
        let req_receiver = self.req_receiver.clone();
        let cmd_sender = self.cmd_sender.clone();

        idle_count.fetch_add(1, Ordering::Relaxed);
        tokio::spawn(async move {
            match task.run(req_receiver).await {
                Ok(_) => {
                    idle_count.fetch_sub(1, Ordering::Relaxed);
                    let _ = cmd_sender.send(PoolCommand::ConnectionClosed).await;
                }
                Err(e) => {
                    warn!("connect to ldap server failed: {e}");
                    idle_count.fetch_sub(1, Ordering::Relaxed);
                    let _ = cmd_sender.send(PoolCommand::ConnectFailed).await;
                }
            }
        });
    }
}
