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

use std::io;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::oneshot;

pub async fn wait_eof<S>(
    mut ctl_stream: S,
    mut ctl_close_sender: oneshot::Sender<Option<io::Error>>,
) where
    S: AsyncRead + AsyncWrite + Unpin,
{
    const MAX_MSG_SIZE: usize = 4;
    let mut buf = [0u8; MAX_MSG_SIZE];

    loop {
        tokio::select! {
            biased;

            r = ctl_stream.read(&mut buf) => {
                let e = match r {
                    Ok(0) => None,
                    Ok(MAX_MSG_SIZE) => {
                        let _ = ctl_stream.shutdown().await;
                        Some(io::Error::other("unexpected data received in the control connection"))
                    }
                    Ok(_) => continue, // Some bad implementations may send extra data to keep alive
                    Err(e) => Some(e),
                };
                let _ = ctl_close_sender.send(e);
                break;
            }
            _ = ctl_close_sender.closed() => {
                let _ = ctl_stream.shutdown().await;
                break;
            }
        }
    }
}
