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

use std::io::{self, Error, Write};

use async_trait::async_trait;
use tokio::io::{AsyncWrite, AsyncWriteExt, BufWriter};

use g3_ftp_client::FtpLineDataReceiver;

const CHUNKED_BUF_HEAD_RESERVED: usize = (usize::BITS as usize >> 2) + 2;
const CHUNKED_BUF_TAIL_RESERVED: usize = 2;

#[async_trait]
pub(super) trait ListWriter: FtpLineDataReceiver {
    fn take_io_error(&mut self) -> Option<io::Error>;
    async fn flush_buf(&mut self) -> io::Result<()>;
    fn is_idle(&self) -> bool;
    fn reset_active(&mut self);
    fn no_cached_data(&self) -> bool;
}

pub(super) struct ChunkedListWriter<'a, W> {
    buf_len: usize,
    buf_cap: usize,
    buf: Vec<u8>,
    writer: &'a mut W,
    io_error: Option<io::Error>,
    active: bool,
}

impl<'a, W> ChunkedListWriter<'a, W>
where
    W: AsyncWrite + Unpin,
{
    pub(super) fn new(writer: &'a mut W, buf_size: usize) -> Self {
        let mut buf =
            Vec::with_capacity(CHUNKED_BUF_HEAD_RESERVED + buf_size + CHUNKED_BUF_TAIL_RESERVED);
        buf.extend_from_slice(&[0u8; CHUNKED_BUF_HEAD_RESERVED]);
        ChunkedListWriter {
            buf_len: CHUNKED_BUF_HEAD_RESERVED,
            buf_cap: buf_size + CHUNKED_BUF_HEAD_RESERVED,
            buf,
            writer,
            io_error: None,
            active: false,
        }
    }

    async fn send_buf(&mut self) -> io::Result<()> {
        let chunked_header = format!("{:x}\r\n", self.buf_len - CHUNKED_BUF_HEAD_RESERVED);
        let offset = CHUNKED_BUF_HEAD_RESERVED - chunked_header.len();
        let mut head = &mut self.buf[offset..];
        let _ = head.write_all(chunked_header.as_bytes());
        self.buf.extend_from_slice(b"\r\n");
        self.writer.write_all(&self.buf[offset..]).await?;

        self.buf_cap = self.buf.capacity() - CHUNKED_BUF_TAIL_RESERVED;
        self.buf_len = CHUNKED_BUF_HEAD_RESERVED;
        self.buf.truncate(self.buf_len);
        Ok(())
    }
}

#[async_trait]
impl<'a, W> FtpLineDataReceiver for ChunkedListWriter<'a, W>
where
    W: AsyncWrite + Send + Unpin,
{
    async fn recv_line(&mut self, line: &str) {
        self.active = true;

        if self.buf_cap - self.buf_len < line.len() {
            if let Err(e) = self.send_buf().await {
                self.io_error = Some(e);
                return;
            }
        }

        self.buf.extend_from_slice(line.as_bytes());
        self.buf_len += line.len();
    }

    #[inline]
    fn should_return_early(&self) -> bool {
        self.io_error.is_some()
    }
}

#[async_trait]
impl<'a, W> ListWriter for ChunkedListWriter<'a, W>
where
    W: AsyncWrite + Send + Unpin,
{
    #[inline]
    fn take_io_error(&mut self) -> Option<Error> {
        self.io_error.take()
    }

    async fn flush_buf(&mut self) -> io::Result<()> {
        if self.buf_len > CHUNKED_BUF_HEAD_RESERVED {
            self.send_buf().await?;
        }
        self.writer.write_all(b"0\r\n\r\n").await?;
        self.writer.flush().await
    }

    #[inline]
    fn is_idle(&self) -> bool {
        !self.active
    }

    #[inline]
    fn reset_active(&mut self) {
        self.active = false;
    }

    #[inline]
    fn no_cached_data(&self) -> bool {
        self.buf_len <= CHUNKED_BUF_HEAD_RESERVED
    }
}

pub(super) struct EndingListWriter<'a, W> {
    writer: BufWriter<&'a mut W>,
    io_error: Option<io::Error>,
    active: bool,
}

impl<'a, W> EndingListWriter<'a, W>
where
    W: AsyncWrite + Unpin,
{
    pub(super) fn new(writer: &'a mut W, buf_size: usize) -> Self {
        EndingListWriter {
            writer: BufWriter::with_capacity(buf_size, writer),
            io_error: None,
            active: false,
        }
    }
}

#[async_trait]
impl<'a, W> FtpLineDataReceiver for EndingListWriter<'a, W>
where
    W: AsyncWrite + Send + Unpin,
{
    async fn recv_line(&mut self, line: &str) {
        self.active = true;
        if let Err(e) = self.writer.write_all(line.as_bytes()).await {
            self.io_error = Some(e);
        }
    }

    #[inline]
    fn should_return_early(&self) -> bool {
        self.io_error.is_some()
    }
}

#[async_trait]
impl<'a, W> ListWriter for EndingListWriter<'a, W>
where
    W: AsyncWrite + Send + Unpin,
{
    #[inline]
    fn take_io_error(&mut self) -> Option<io::Error> {
        self.io_error.take()
    }

    #[inline]
    async fn flush_buf(&mut self) -> io::Result<()> {
        self.writer.flush().await
    }

    #[inline]
    fn is_idle(&self) -> bool {
        !self.active
    }

    #[inline]
    fn reset_active(&mut self) {
        self.active = false;
    }

    #[inline]
    fn no_cached_data(&self) -> bool {
        self.writer.buffer().is_empty()
    }
}
