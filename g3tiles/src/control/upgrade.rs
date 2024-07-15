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

use anyhow::anyhow;
use log::warn;
use tokio::sync::{mpsc, oneshot};

use g3tiles_proto::proc_capnp::proc_control;
use g3tiles_proto::types_capnp::operation_result;

use g3_daemon::control::LocalController;

static MSG_CHANNEL: Mutex<Option<mpsc::Sender<Msg>>> = Mutex::new(None);
static THREAD_HANDLE: Mutex<Option<JoinHandle<()>>> = Mutex::new(None);

enum Msg {
    CancelShutdown,
    ReleaseController(oneshot::Sender<()>),
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
        drop(sender);
        let handle = THREAD_HANDLE.lock().unwrap().take();
        if let Some(handle) = handle {
            tokio::task::spawn_blocking(move || {
                let _ = handle.join();
            });
        }
    }
}

pub fn connect_to_old_daemon() {
    let (sender, receiver) = mpsc::channel(4);
    let handle = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_io()
            .build()
            .unwrap();
        rt.block_on(async {
            if let Err(e) = connect_run(receiver).await {
                warn!("upgrade channel error: {e}");
            }
        })
    });
    let mut msg_channel = MSG_CHANNEL.lock().unwrap();
    *msg_channel = Some(sender);
    let mut thread_handle = THREAD_HANDLE.lock().unwrap();
    *thread_handle = Some(handle);
}

async fn connect_run(mut msg_receiver: mpsc::Receiver<Msg>) -> anyhow::Result<()> {
    let (rpc_system, proc_control) = LocalController::connect_rpc::<proc_control::Client>(
        crate::build::PKG_NAME,
        crate::opts::daemon_group(),
    )
    .await?;
    tokio::task::LocalSet::new()
        .run_until(async move {
            tokio::task::spawn_local(async move {
                rpc_system
                    .await
                    .map_err(|e| warn!("upgrade rpc system error: {e:?}"))
            });

            while let Some(msg) = msg_receiver.recv().await {
                match msg {
                    Msg::CancelShutdown => {
                        let req = proc_control.cancel_shutdown_request();
                        let rsp = req.send().promise.await?;
                        return check_operation_result(rsp.get()?.get_result()?);
                    }
                    Msg::ReleaseController(finish_sender) => {
                        let req = proc_control.release_controller_request();
                        let rsp = req.send().promise.await?;
                        check_operation_result(rsp.get()?.get_result()?)?;
                        let _ = finish_sender.send(());
                    }
                }
            }

            Ok(())
        })
        .await
}

fn check_operation_result(r: operation_result::Reader<'_>) -> anyhow::Result<()> {
    match r.which().unwrap() {
        operation_result::Which::Ok(_) => Ok(()),
        operation_result::Which::Err(err) => {
            let e = err?;
            let msg = e.get_reason()?.to_str()?;
            Err(anyhow!("remote error: {} - {msg}", e.get_code()))
        }
    }
}
