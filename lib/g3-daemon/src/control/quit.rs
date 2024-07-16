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

use std::future::Future;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::anyhow;
use log::{debug, warn};
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

pub trait QuitAction: Sized + Send {
    fn release_controller(&self) -> impl Future<Output = ()> + Send;
    fn resume_controller(&self) -> anyhow::Result<()>;
    fn do_graceful_shutdown(&self) -> impl Future<Output = ()> + Send;
    fn do_force_shutdown(&self) -> impl Future<Output = ()> + Send;

    #[allow(async_fn_in_trait)]
    async fn into_running(self, graceful_wait: Duration) {
        let (msg_sender, mut msg_receiver) = mpsc::channel(4);
        set_daemon_quit_channel(msg_sender);

        let mut controller_released = false;

        'outer: while let Some(msg) = msg_receiver.recv().await {
            match msg {
                Command::StartGracefulShutdown => {
                    debug!("will start graceful shutdown after {graceful_wait:?}");
                    loop {
                        match tokio::time::timeout(graceful_wait, msg_receiver.recv()).await {
                            Ok(Some(Command::StartGracefulShutdown)) => continue,
                            Ok(Some(Command::ReleaseController(finish_sender))) => {
                                self.release_controller().await;
                                let _ = finish_sender.send(true);
                                controller_released = true;
                                continue;
                            }
                            Ok(Some(Command::CancelGracefulShutdown(finish_sender))) => {
                                if controller_released {
                                    match self.resume_controller() {
                                        Ok(_) => controller_released = false,
                                        Err(e) => {
                                            warn!("failed to resume daemon controller: {e}");
                                        }
                                    }
                                }
                                let _ = finish_sender.send(!controller_released);
                                debug!("graceful shutdown canceled");
                                continue 'outer;
                            }
                            Ok(None) => break 'outer,
                            Err(_) => {
                                self.release_controller().await;
                                break;
                            }
                        }
                    }
                    debug!("start graceful shutdown now");
                    self.do_graceful_shutdown().await;
                    return;
                }
                Command::ReleaseController(finish_sender) => {
                    let _ = finish_sender.send(false);
                }
                Command::CancelGracefulShutdown(finish_sender) => {
                    let _ = finish_sender.send(true);
                }
            }
        }

        self.release_controller().await;
        debug!("start force shutdown");
        self.do_force_shutdown().await;
        clear_daemon_quit_channel();
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
