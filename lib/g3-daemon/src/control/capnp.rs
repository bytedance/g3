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

use std::sync::Mutex;

use anyhow::anyhow;
use capnp_rpc::{rpc_twoparty_capnp, twoparty, RpcSystem};
use log::{trace, warn};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::{mpsc, oneshot};

static CAPNP_MESSAGE_SENDER: Mutex<Option<mpsc::UnboundedSender<CapnpMessage>>> = Mutex::new(None);

struct CapnpMessage {
    reader: Box<dyn AsyncRead + Send + Unpin>,
    writer: Box<dyn AsyncWrite + Send + Unpin>,
}

pub fn handle_capnp_connection<R, W>(reader: R, writer: W) -> anyhow::Result<()>
where
    R: AsyncRead + Send + Unpin + 'static,
    W: AsyncWrite + Send + Unpin + 'static,
{
    let msg = CapnpMessage {
        reader: Box::new(reader),
        writer: Box::new(writer),
    };
    let value = CAPNP_MESSAGE_SENDER.lock().unwrap();
    if let Some(sender) = &*value {
        sender
            .send(msg)
            .map_err(|_| anyhow!("failed to send msg to capnp thread"))?;
        Ok(())
    } else {
        Err(anyhow!("no sender to capnp thread available"))
    }
}

fn set_capnp_message_sender(sender: mpsc::UnboundedSender<CapnpMessage>) {
    let mut value = CAPNP_MESSAGE_SENDER.lock().unwrap();
    *value = Some(sender);
}

pub fn stop_working_thread() {
    let mut value = CAPNP_MESSAGE_SENDER.lock().unwrap();
    *value = None;
}

pub async fn spawn_working_thread<F>(
    build_client: &'static F,
) -> anyhow::Result<std::thread::JoinHandle<()>>
where
    F: Fn() -> capnp::capability::Client + Sync,
{
    let (sender, receiver) = mpsc::unbounded_channel::<CapnpMessage>();
    let (ready_notifier, ready) = oneshot::channel::<bool>();
    let handler = std::thread::Builder::new()
        .name("capnp_ctl".to_string())
        .spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();
            tokio::task::LocalSet::new().block_on(&rt, async move {
                let mut receiver = receiver;
                set_capnp_message_sender(sender);
                ready_notifier.send(true).unwrap();
                while let Some(msg) = receiver.recv().await {
                    trace!("received new capnp rpc connection");
                    let reader = msg.reader;
                    let writer = msg.writer;
                    let reader = tokio_util::compat::TokioAsyncReadCompatExt::compat(reader);
                    let writer = tokio_util::compat::TokioAsyncWriteCompatExt::compat_write(writer);

                    let network = twoparty::VatNetwork::new(
                        reader,
                        writer,
                        rpc_twoparty_capnp::Side::Server,
                        Default::default(),
                    );

                    let client = build_client();
                    let rpc_system = RpcSystem::new(Box::new(network), Some(client));
                    tokio::task::spawn_local(async move {
                        trace!("handling capnp rpc connection ...");
                        if let Err(e) = rpc_system.await {
                            warn!("capnp failed: {e:?}");
                        } else {
                            trace!("capnp success");
                        }
                    });
                }
            });
        })
        .map_err(|e| anyhow!("failed to spawn thread: {e:?}"))?;
    match ready.await {
        Ok(true) => Ok(handler),
        Ok(false) => {
            let _ = handler.join();
            Err(anyhow!("capnp ctl thread is not ready"))
        }
        Err(e) => {
            let _ = handler.join();
            Err(anyhow!("failed to recv ready signal: {e:?}"))
        }
    }
}
