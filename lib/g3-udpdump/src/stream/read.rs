/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use tokio::io::{AsyncRead, ReadBuf};
use tokio::sync::mpsc;

use super::{
    PduHeader, ProxyToClientPduHeader, ProxyToRemotePduHeader, StreamDumpState, ToClientPduHeader,
    ToRemotePduHeader,
};

pub type FromClientStreamDumpReader<W> = StreamDumpReader<W, ToRemotePduHeader>;
pub type FromRemoteStreamDumpReader<W> = StreamDumpReader<W, ToClientPduHeader>;
pub type ProxyFromClientStreamDumpReader<W> = StreamDumpReader<W, ProxyToRemotePduHeader>;
pub type ProxyFromRemoteStreamDumpReader<W> = StreamDumpReader<W, ProxyToClientPduHeader>;

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
