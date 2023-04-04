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

use tokio::io::AsyncWriteExt;
use tokio::sync::oneshot;

use g3_io_ext::LimitedBufReadExt;

use super::BoxHttpForwardConnection;

struct HttpConnectionEofCheck {
    conn: BoxHttpForwardConnection,
    wait_channel: oneshot::Receiver<bool>,
    send_channel: oneshot::Sender<BoxHttpForwardConnection>,
}

impl HttpConnectionEofCheck {
    async fn run(self) {
        let HttpConnectionEofCheck {
            mut conn,
            mut wait_channel,
            send_channel,
        } = self;
        tokio::select! {
            biased;

            _ = conn.1.fill_wait_eof() => {
                // close early to avoid waiting at other side
                wait_channel.close();
                // make sure we correctly shutdown tls connection
                // FIXME use async drop at escaper side when supported
                let _ = conn.0.shutdown().await;
            }
            v = &mut wait_channel => {
                if matches!(v, Ok(true)) {
                    let _ = send_channel.send(conn);
                } else {
                    // make sure we correctly shutdown tls connection
                    // FIXME use async drop at escaper side when supported
                    let _ = conn.0.shutdown().await;
                }
            }
        }
    }
}

pub(crate) struct HttpConnectionEofPoller {
    notify_channel: oneshot::Sender<bool>,
    recv_channel: oneshot::Receiver<BoxHttpForwardConnection>,
}

impl HttpConnectionEofPoller {
    pub(crate) fn spawn(conn: BoxHttpForwardConnection) -> Self {
        let (notify_sender, notify_receiver) = oneshot::channel();
        let (conn_sender, conn_receiver) = oneshot::channel();
        let runtime = HttpConnectionEofCheck {
            conn,
            wait_channel: notify_receiver,
            send_channel: conn_sender,
        };
        tokio::spawn(runtime.run());
        HttpConnectionEofPoller {
            notify_channel: notify_sender,
            recv_channel: conn_receiver,
        }
    }

    pub(crate) async fn recv_conn(self) -> Option<BoxHttpForwardConnection> {
        self.notify_channel.send(true).ok()?;
        match self.recv_channel.await {
            Ok(conn) => Some(conn),
            Err(_) => None,
        }
    }
}
