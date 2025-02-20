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
use std::thread::JoinHandle;

use capnp_rpc::{RpcSystem, rpc_twoparty_capnp};
use log::warn;
use tokio::sync::{mpsc, oneshot};

static MSG_CHANNEL: Mutex<Option<mpsc::Sender<Msg>>> = Mutex::new(None);
static THREAD_HANDLE: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

enum Msg {
    CancelShutdown,
    ReleaseController(oneshot::Sender<()>),
    ConfirmShutdown,
}

pub fn cancel_old_shutdown() {
    let msg_channel = MSG_CHANNEL.lock().unwrap().take();
    if let Some(sender) = msg_channel {
        let _ = sender.try_send(Msg::CancelShutdown);
        let handle = THREAD_HANDLE.lock().unwrap().take();
        if let Some(handle) = handle {
            let _ = handle.join();
        }
    }
}

pub async fn release_old_controller() {
    let msg_channel = MSG_CHANNEL.lock().unwrap().clone();
    if let Some(sender) = msg_channel {
        let (done_sender, done_receiver) = oneshot::channel();
        if sender
            .send(Msg::ReleaseController(done_sender))
            .await
            .is_ok()
        {
            let _ = done_receiver.await;
        }
    }
}

pub fn finish() {
    let msg_channel = MSG_CHANNEL.lock().unwrap().take();
    if let Some(sender) = msg_channel {
        let _ = sender.try_send(Msg::ConfirmShutdown);
        let handle = THREAD_HANDLE.lock().unwrap().take();
        if let Some(handle) = handle {
            tokio::task::spawn_blocking(move || {
                let _ = handle.join();
            });
        }
    }
}

pub trait UpgradeAction: Sized {
    #[allow(async_fn_in_trait)]
    async fn connect_rpc() -> anyhow::Result<(RpcSystem<rpc_twoparty_capnp::Side>, Self)>;
    #[allow(async_fn_in_trait)]
    async fn cancel_shutdown(&self) -> anyhow::Result<()>;
    #[allow(async_fn_in_trait)]
    async fn release_controller(&self) -> anyhow::Result<()>;
    #[allow(async_fn_in_trait)]
    async fn confirm_shutdown(&self) -> anyhow::Result<()>;

    fn connect_to_old_daemon() {
        let (sender, receiver) = mpsc::channel(4);
        let handle = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_io()
                .build()
                .unwrap();
            rt.block_on(async {
                if let Err(e) = connect_run::<Self>(receiver).await {
                    warn!("upgrade channel error: {e}");
                }
            })
        });
        let mut msg_channel = MSG_CHANNEL.lock().unwrap();
        *msg_channel = Some(sender);
        let mut thread_handle = THREAD_HANDLE.lock().unwrap();
        *thread_handle = Some(handle);
    }
}

async fn connect_run<T: UpgradeAction>(
    mut msg_receiver: mpsc::Receiver<Msg>,
) -> anyhow::Result<()> {
    let (rpc_system, action) = T::connect_rpc().await?;
    tokio::task::LocalSet::new()
        .run_until(async move {
            tokio::task::spawn_local(async move {
                rpc_system
                    .await
                    .map_err(|e| warn!("upgrade rpc system error: {e:?}"))
            });

            while let Some(msg) = msg_receiver.recv().await {
                match msg {
                    Msg::CancelShutdown => return action.cancel_shutdown().await,
                    Msg::ReleaseController(finish_sender) => {
                        if let Err(e) = action.release_controller().await {
                            warn!("ReleaseController upgrade request failed: {e}");
                        }
                        let _ = finish_sender.send(());
                    }
                    Msg::ConfirmShutdown => return action.confirm_shutdown().await,
                }
            }

            Ok(())
        })
        .await
}
