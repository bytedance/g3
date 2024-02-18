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

use std::io::{self, IoSlice};
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use tokio::io::AsyncWrite;
use tokio::sync::mpsc;

use super::{PduHeader, StreamDumpState, ToClientPduHeader, ToRemotePduHeader};

pub type ToClientStreamDumpWriter<W> = StreamDumpWriter<W, ToClientPduHeader>;
pub type ToRemoteStreamDumpWriter<W> = StreamDumpWriter<W, ToRemotePduHeader>;

pub struct StreamDumpWriter<W, H> {
    writer: W,
    state: StreamDumpState<H>,
}

impl<W: AsyncWrite, H: PduHeader> StreamDumpWriter<W, H> {
    pub(super) fn new(
        writer: W,
        header: H,
        sender: mpsc::UnboundedSender<Vec<u8>>,
        pkt_size: usize,
    ) -> Self {
        let state = StreamDumpState::new(header, sender, pkt_size);
        StreamDumpWriter { writer, state }
    }
}

impl<W: AsyncWrite + Unpin, H: PduHeader + Unpin> AsyncWrite for StreamDumpWriter<W, H> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let nw = ready!(Pin::new(&mut self.writer).poll_write(cx, buf))?;
        self.get_mut().state.dump_all_buf(buf);
        Poll::Ready(Ok(nw))
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.writer).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.writer).poll_shutdown(cx)
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        let count = ready!(Pin::new(&mut self.writer).poll_write_vectored(cx, bufs))?;
        self.get_mut().state.dump_all_bufs(&bufs[0..count]);
        Poll::Ready(Ok(count))
    }

    fn is_write_vectored(&self) -> bool {
        self.writer.is_write_vectored()
    }
}
