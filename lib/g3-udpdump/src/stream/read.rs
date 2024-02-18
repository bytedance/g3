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
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use tokio::io::{AsyncRead, ReadBuf};
use tokio::sync::mpsc;

use super::{PduHeader, StreamDumpState, ToClientPduHeader, ToRemotePduHeader};

pub type FromClientStreamDumpReader<W> = StreamDumpReader<W, ToRemotePduHeader>;
pub type FromRemoteStreamDumpReader<W> = StreamDumpReader<W, ToClientPduHeader>;

pub struct StreamDumpReader<R, H> {
    reader: R,
    state: StreamDumpState<H>,
}

impl<R: AsyncRead, H: PduHeader> StreamDumpReader<R, H> {
    pub(super) fn new(
        reader: R,
        header: H,
        sender: mpsc::UnboundedSender<Vec<u8>>,
        pkt_size: usize,
    ) -> Self {
        let state = StreamDumpState::new(header, sender, pkt_size);
        StreamDumpReader { reader, state }
    }
}

impl<R: AsyncRead + Unpin, H: PduHeader + Unpin> AsyncRead for StreamDumpReader<R, H> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let offset = buf.filled().len();
        ready!(Pin::new(&mut self.reader).poll_read(cx, buf))?;
        let filled = buf.filled();
        if filled.len() > offset {
            self.get_mut().state.dump_all_buf(&filled[offset..]);
        }
        Poll::Ready(Ok(()))
    }
}
