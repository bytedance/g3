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

use std::sync::Mutex;
use std::time::Duration;

use anyhow::anyhow;
use log::{info, warn};
use tokio::sync::{mpsc, oneshot};

static DAEMON_QUIT_CHANNEL: Mutex<Option<mpsc::Sender<Command>>> = Mutex::new(None);

enum Command {
    StartGracefulShutdown,
    ReleaseController(oneshot::Sender<bool>),
    CancelGracefulShutdown(oneshot::Sender<bool>),
}

async fn send_command(msg: Command) {
    let mut channel = DAEMON_QUIT_CHANNEL.lock().unwrap().clone();
    if let Some(channel) = channel.take() {
        let _ = channel.send(msg).await;
    }
}

pub async fn start_graceful_shutdown() {
    send_command(Command::StartGracefulShutdown).await;
}

pub async fn release_controller() -> anyhow::Result<()> {
    let (sender, receiver) = oneshot::channel();
    send_command(Command::ReleaseController(sender)).await;
    match receiver.await {
        Ok(true) => Ok(()),
        Ok(false) => Err(anyhow!("not in graceful shutdown state")),
        Err(_) => Err(anyhow!("channel closed unexpectedly")),
    }
}

pub async fn cancel_graceful_shutdown() -> anyhow::Result<()> {
    let (sender, receiver) = oneshot::channel();
    send_command(Command::CancelGracefulShutdown(sender)).await;
    match receiver.await {
        Ok(true) => Ok(()),
        Ok(false) => Err(anyhow!("failed to resume controller")),
        Err(_) => Err(anyhow!("channel closed unexpectedly")),
    }
}

pub fn trigger_force_shutdown() {
    let mut channel = DAEMON_QUIT_CHANNEL.lock().unwrap();
    *channel = None;
}

pub trait QuitAction: Default {
    fn do_release_controller(&self) -> impl Future<Output = ()> + Send;
    fn do_resume_controller(&self) -> anyhow::Result<()>;
    fn do_graceful_shutdown(&self) -> impl Future<Output = ()> + Send;
    fn do_force_shutdown(&self) -> impl Future<Output = ()> + Send;

    fn tokio_spawn_run()
    where
        Self: Send + Sync + 'static,
    {
        let actor = Self::default();
        tokio::spawn(QuitLoop::new(actor).run());
    }
}

struct QuitLoop<T: QuitAction> {
    action: T,
    graceful_wait: Duration,
    controller_released: bool,
    msg_receiver: mpsc::Receiver<Command>,
}

impl<T: QuitAction> QuitLoop<T> {
    fn new(action: T) -> Self {
        let (msg_sender, msg_receiver) = mpsc::channel(4);
        set_daemon_quit_channel(msg_sender);

        QuitLoop {
            action,
            graceful_wait: crate::runtime::config::get_server_offline_delay(),
            controller_released: false,
            msg_receiver,
        }
    }

    async fn run(mut self) {
        'outer: while let Some(msg) = self.msg_receiver.recv().await {
            match msg {
                Command::StartGracefulShutdown => {
                    // this should be sent by an init, which doesn't know it's a restart or stop
                    info!("received StartGracefulShutdown request");
                    let mut is_restart = false;
                    loop {
                        info!(
                            "will start graceful shutdown after {:?}",
                            self.graceful_wait
                        );
                        match tokio::time::timeout(self.graceful_wait, self.msg_receiver.recv())
                            .await
                        {
                            Ok(Some(Command::StartGracefulShutdown)) => {
                                if is_restart {
                                    // the new process tell us to stop
                                    break;
                                }
                            }
                            Ok(Some(Command::ReleaseController(finish_sender))) => {
                                // this should be sent by the new process, now we know it's a restart
                                self.release_controller().await;
                                let _ = finish_sender.send(true);
                                is_restart = true;
                            }
                            Ok(Some(Command::CancelGracefulShutdown(finish_sender))) => {
                                // the new process tell us to stop
                                info!("received CancelGracefulShutdown request");
                                let resumed = self.resume_controller().await;
                                let _ = finish_sender.send(resumed);
                                info!("graceful shutdown canceled");
                                continue 'outer;
                            }
                            Ok(None) => break 'outer,
                            Err(_) => {
                                if is_restart {
                                    // the new process may be failed to start, so we consume
                                    info!("timeout to wait StartGracefulShutdown request, will resume");
                                    self.resume_controller().await;
                                    info!("graceful shutdown canceled");
                                    continue 'outer;
                                } else {
                                    // treat timeout to stop
                                    info!("timeout to wait new request, will stop");
                                    self.release_controller().await;
                                    break;
                                }
                            }
                        }
                    }
                    self.graceful_shutdown().await;
                    return;
                }
                Command::ReleaseController(finish_sender) => {
                    // this should be sent by the new process, which knows it's a restart
                    info!("received ReleaseController request");
                    self.release_controller().await;
                    let _ = finish_sender.send(true);
                    loop {
                        info!(
                            "will start graceful shutdown after {:?}",
                            self.graceful_wait
                        );
                        match tokio::time::timeout(self.graceful_wait, self.msg_receiver.recv())
                            .await
                        {
                            Ok(Some(Command::StartGracefulShutdown)) => {
                                // the new process tell us to stop
                                break;
                            }
                            Ok(Some(Command::ReleaseController(finish_sender))) => {
                                let _ = finish_sender.send(true);
                            }
                            Ok(Some(Command::CancelGracefulShutdown(finish_sender))) => {
                                // the new process tell us to resume
                                info!("received CancelGracefulShutdown request");
                                let resumed = self.resume_controller().await;
                                let _ = finish_sender.send(resumed);
                                info!("graceful shutdown canceled");
                                continue 'outer;
                            }
                            Ok(None) => break 'outer,
                            Err(_) => {
                                // the new process may be failed to start, so we consume
                                info!("timeout to wait StartGracefulShutdown request, will resume");
                                self.resume_controller().await;
                                info!("graceful shutdown canceled");
                                continue 'outer;
                            }
                        }
                    }
                    self.graceful_shutdown().await;
                    return;
                }
                Command::CancelGracefulShutdown(finish_sender) => {
                    let _ = finish_sender.send(true);
                }
            }
        }

        self.release_controller().await;
        info!("start force shutdown");
        self.action.do_force_shutdown().await;
        clear_daemon_quit_channel();
    }

    async fn graceful_shutdown(&self) {
        info!("start graceful shutdown now");
        self.action.do_graceful_shutdown().await;
    }

    async fn release_controller(&mut self) {
        if self.controller_released {
            return;
        }
        self.action.do_release_controller().await;
        self.controller_released = true;
        info!("daemon controller released");
    }

    async fn resume_controller(&mut self) -> bool {
        if self.controller_released {
            match self.action.do_resume_controller() {
                Ok(_) => {
                    self.controller_released = false;
                    info!("daemon controller resumed");
                    true
                }
                Err(e) => {
                    warn!("failed to resume daemon controller: {e}");
                    false
                }
            }
        } else {
            true
        }
    }
}

fn set_daemon_quit_channel(msg_sender: mpsc::Sender<Command>) {
    let mut channel = DAEMON_QUIT_CHANNEL.lock().unwrap();
    *channel = Some(msg_sender);
}

fn clear_daemon_quit_channel() {
    let mut channel = DAEMON_QUIT_CHANNEL.lock().unwrap();
    let _ = channel.take();
}
