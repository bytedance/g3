/*
 * SPDX-License-Identifier: Apache-2.0
 * SPDX-FileCopyrightText: 2024-2025 ByteDance and/or its affiliates.
 * SPDX-FileCopyrightText: 2026 VEY-OSS Developers.
 */

use std::io::IoSlice;
use std::mem;

use tokio::sync::mpsc;

use super::PduHeader;

pub struct StreamDumpState<H> {
    header: H,
    sender: mpsc::UnboundedSender<Vec<u8>>,
    buf: Vec<u8>,
    pkt_size: usize,
    hdr_len: usize,
}

impl<H: PduHeader> StreamDumpState<H> {
    pub(crate) fn new(
        mut header: H,
        sender: mpsc::UnboundedSender<Vec<u8>>,
        mut pkt_size: usize,
    ) -> Self {
        pkt_size = pkt_size.max(1200);
        let buf = header.new_header(pkt_size);
        let hdr_len = buf.len();
        StreamDumpState {
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

    pub(crate) fn dump_all_buf(&mut self, buf: &[u8]) {
        self.dump_buf(buf);
        if self.has_pending_data() {
            self.flush_data();
        }
    }

    /// Dump the first `nbytes` of application data across `bufs` (byte prefix, not
    /// slice-count prefix). This matches `AsyncWrite::poll_write_vectored`'s return value.
    pub(crate) fn dump_all_bufs(&mut self, bufs: &[IoSlice<'_>], mut nbytes: usize) {
        for buf in bufs {
            if nbytes == 0 {
                break;
            }
            let take = nbytes.min(buf.len());
            self.dump_buf(&buf[..take]);
            nbytes -= take;
        }
        if self.has_pending_data() {
            self.flush_data();
        }
    }
}
