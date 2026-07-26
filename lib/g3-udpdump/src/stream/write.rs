/*
 * SPDX-License-Identifier: Apache-2.0
 * SPDX-FileCopyrightText: 2024-2025 ByteDance and/or its affiliates.
 * SPDX-FileCopyrightText: 2026 VEY-OSS Developers.
 */

use std::io::{self, IoSlice};
use std::pin::Pin;
use std::task::{Context, Poll, ready};

use tokio::io::AsyncWrite;
use tokio::sync::mpsc;

use super::{
    PduHeader, ProxyToClientPduHeader, ProxyToRemotePduHeader, StreamDumpState, ToClientPduHeader,
    ToRemotePduHeader,
};

pub type ToClientStreamDumpWriter<W> = StreamDumpWriter<W, ToClientPduHeader>;
pub type ToRemoteStreamDumpWriter<W> = StreamDumpWriter<W, ToRemotePduHeader>;
pub type ProxyToClientStreamDumpWriter<W> = StreamDumpWriter<W, ProxyToClientPduHeader>;
pub type ProxyToRemoteStreamDumpWriter<W> = StreamDumpWriter<W, ProxyToRemotePduHeader>;

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
        // Only dump bytes that were actually accepted by the inner writer.
        self.get_mut().state.dump_all_buf(&buf[..nw]);
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
        let nw = ready!(Pin::new(&mut self.writer).poll_write_vectored(cx, bufs))?;
        // `nw` is a byte count, not an IoSlice count — dump the byte prefix.
        self.get_mut().state.dump_all_bufs(bufs, nw);
        Poll::Ready(Ok(nw))
    }

    fn is_write_vectored(&self) -> bool {
        self.writer.is_write_vectored()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::{IpAddr, Ipv4Addr, SocketAddr};
    use std::task::Poll;

    use tokio::io::AsyncWriteExt;
    use tokio::sync::mpsc;

    use crate::ExportedPduDissectorHint;
    use crate::stream::header;

    /// Inner writer that accepts at most `max_per_write` bytes per call.
    struct PartialWriter {
        written: Vec<u8>,
        max_per_write: usize,
    }

    impl AsyncWrite for PartialWriter {
        fn poll_write(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            buf: &[u8],
        ) -> Poll<io::Result<usize>> {
            let n = buf.len().min(self.max_per_write);
            self.written.extend_from_slice(&buf[..n]);
            Poll::Ready(Ok(n))
        }

        fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_write_vectored(
            mut self: Pin<&mut Self>,
            _cx: &mut Context<'_>,
            bufs: &[IoSlice<'_>],
        ) -> Poll<io::Result<usize>> {
            let mut left = self.max_per_write;
            let mut total = 0;
            for buf in bufs {
                if left == 0 {
                    break;
                }
                let n = buf.len().min(left);
                self.written.extend_from_slice(&buf[..n]);
                left -= n;
                total += n;
            }
            Poll::Ready(Ok(total))
        }

        fn is_write_vectored(&self) -> bool {
            true
        }
    }

    fn new_dump_writer(
        max_per_write: usize,
    ) -> (
        StreamDumpWriter<PartialWriter, header::ToRemotePduHeader>,
        mpsc::UnboundedReceiver<Vec<u8>>,
    ) {
        let (sender, receiver) = mpsc::unbounded_channel();
        let client = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 1234);
        let remote = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 80);
        let (_to_c, to_r) = header::new_pair(client, remote, ExportedPduDissectorHint::TcpPort(80));
        let writer = StreamDumpWriter::new(
            PartialWriter {
                written: Vec::new(),
                max_per_write,
            },
            to_r,
            sender,
            1200,
        );
        (writer, receiver)
    }

    #[tokio::test]
    async fn poll_write_dumps_only_accepted_bytes() {
        let (mut writer, mut receiver) = new_dump_writer(5);
        let data = b"ABCDEFGHIJKLMNOP";

        let nw = writer.write(data).await.unwrap();
        assert_eq!(nw, 5);
        assert_eq!(&writer.writer.written, b"ABCDE");

        let pkt = receiver.try_recv().expect("should dump one PDU");
        assert!(
            pkt.windows(5).any(|w| w == b"ABCDE"),
            "dump should contain the accepted prefix"
        );
        assert!(
            !pkt.windows(6).any(|w| w == b"ABCDEF"),
            "dump must not contain bytes beyond what the inner writer accepted"
        );
    }

    #[tokio::test]
    async fn poll_write_vectored_dumps_byte_prefix_not_slice_count() {
        let (mut writer, mut receiver) = new_dump_writer(7);
        let a = b"AAAA";
        let b = b"BBBBBBBB"; // 8 bytes; together with `a` is 12 bytes > max_per_write
        let bufs = [IoSlice::new(a), IoSlice::new(b)];

        // nbytes (7) > bufs.len() (2): the old bug indexed bufs[0..7] and panicked.
        let nw = writer.write_vectored(&bufs).await.unwrap();
        assert_eq!(nw, 7);
        assert_eq!(&writer.writer.written, b"AAAABBB");

        let pkt = receiver.try_recv().expect("should dump one PDU");
        assert!(pkt.windows(7).any(|w| w == b"AAAABBB"));
        assert!(!pkt.windows(8).any(|w| w == b"AAAABBBB"));
    }
}
