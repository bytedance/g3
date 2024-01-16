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
use std::mem;
use std::pin::Pin;
use std::task::{ready, Context, Poll};

use tokio::io::AsyncWrite;
use tokio::sync::mpsc;

use super::{PduHeader, ToClientPduHeader, ToRemotePduHeader};

pub type ToClientStreamDumpWriter<W> = StreamDumpWriter<W, ToClientPduHeader>;
pub type ToRemoteStreamDumpWriter<W> = StreamDumpWriter<W, ToRemotePduHeader>;

pub struct StreamDumpWriter<W, H> {
    writer: W,
    header: H,
    sender: mpsc::UnboundedSender<Vec<u8>>,
    buf: Vec<u8>,
    pkt_size: usize,
    hdr_len: usize,
}

impl<W: AsyncWrite, H: PduHeader> StreamDumpWriter<W, H> {
    pub(super) fn new(
        writer: W,
        mut header: H,
        sender: mpsc::UnboundedSender<Vec<u8>>,
        mut pkt_size: usize,
    ) -> Self {
        pkt_size = pkt_size.max(1200);
        let buf = header.new_header(pkt_size);
        let hdr_len = buf.len();
        StreamDumpWriter {
            writer,
            header,
            sender,
            buf,
            pkt_size,
            hdr_len,
        }
    }

    fn send_data(&mut self, data: &[u8]) -> usize {
        let left = self.pkt_size - self.buf.len();
        if left > data.len() {
            self.buf.extend_from_slice(data);
            data.len()
        } else {
            self.buf.extend_from_slice(&data[0..left]);
            self.flush_data();
            left
        }
    }

    fn has_pending_data(&self) -> bool {
        self.buf.len() > self.hdr_len
    }

    fn flush_data(&mut self) {
        let mut new_buf = Vec::with_capacity(self.pkt_size);
        new_buf.extend_from_slice(&self.buf[0..self.hdr_len]);
        let mut buf = mem::replace(&mut self.buf, new_buf);
        let data_len = buf.len() - self.hdr_len;
        self.header.update_tcp_dissector_data(&mut buf, data_len);
        let _ = self.sender.send(buf);
        self.header.record_written_data(data_len);
    }

    fn dump_buf(&mut self, buf: &[u8]) {
        let mut offset = 0;
        while offset < buf.len() {
            offset += self.send_data(&buf[offset..]);
        }
    }

    fn dump_all_buf(&mut self, buf: &[u8]) {
        self.dump_buf(buf);
        if self.has_pending_data() {
            self.flush_data();
        }
    }

    fn dump_all_bufs(&mut self, bufs: &[IoSlice<'_>]) {
        for buf in bufs {
            self.dump_buf(buf.as_ref());
        }
        if self.has_pending_data() {
            self.flush_data();
        }
    }
}

impl<W: AsyncWrite + Unpin, H: PduHeader + Unpin> AsyncWrite for StreamDumpWriter<W, H> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let nw = ready!(Pin::new(&mut self.writer).poll_write(cx, buf))?;
        self.get_mut().dump_all_buf(buf);
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
        self.get_mut().dump_all_bufs(&bufs[0..count]);
        Poll::Ready(Ok(count))
    }

    fn is_write_vectored(&self) -> bool {
        self.writer.is_write_vectored()
    }
}
