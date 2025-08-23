/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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

            _ = conn.1.fill_wait_data() => {
                // close early when EOF or unexpected data, to avoid waiting at other side
                wait_channel.close();
                let _ = conn.0.shutdown().await;
            }
            v = &mut wait_channel => {
                if matches!(v, Ok(true)) {
                    let _ = send_channel.send(conn);
                } else {
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
        self.recv_channel.await.ok()
    }
}
