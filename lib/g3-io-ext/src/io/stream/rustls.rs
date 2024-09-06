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
use std::io::{Error, IoSlice};
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncRead, AsyncWrite, ReadBuf, ReadHalf, WriteHalf};
use tokio_rustls::{client, server};

use super::AsyncStream;

impl<S> AsyncStream for client::TlsStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    type R = ReadHalf<client::TlsStream<S>>;
    type W = WriteHalf<client::TlsStream<S>>;

    fn into_split(self) -> (Self::R, Self::W) {
        tokio::io::split(self)
    }
}

impl<S> AsyncStream for server::TlsStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    type R = ReadHalf<server::TlsStream<S>>;
    type W = WriteHalf<server::TlsStream<S>>;

    fn into_split(self) -> (Self::R, Self::W) {
        tokio::io::split(self)
    }
}

pub enum MaybeTlsStreamReadHalf<S: AsyncStream> {
    Plain(S::R),
    Tls(ReadHalf<client::TlsStream<S>>),
}

impl<S> AsyncRead for MaybeTlsStreamReadHalf<S>
where
    S: AsyncStream + AsyncRead + AsyncWrite + Unpin,
    S::R: AsyncRead + Unpin,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        match self.get_mut() {
            MaybeTlsStreamReadHalf::Plain(reader) => Pin::new(reader).poll_read(cx, buf),
            MaybeTlsStreamReadHalf::Tls(tls_reader) => Pin::new(tls_reader).poll_read(cx, buf),
        }
    }
}

pub enum MaybeTlsStreamWriteHalf<S: AsyncStream> {
    Plain(S::W),
    Tls(WriteHalf<client::TlsStream<S>>),
}

impl<S> AsyncWrite for MaybeTlsStreamWriteHalf<S>
where
    S: AsyncStream + AsyncRead + AsyncWrite + Unpin,
    S::W: AsyncWrite + Unpin,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, Error>> {
        match self.get_mut() {
            MaybeTlsStreamWriteHalf::Plain(stream) => Pin::new(stream).poll_write(cx, buf),
            MaybeTlsStreamWriteHalf::Tls(tls_stream) => Pin::new(tls_stream).poll_write(cx, buf),
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        match self.get_mut() {
            MaybeTlsStreamWriteHalf::Plain(stream) => Pin::new(stream).poll_flush(cx),
            MaybeTlsStreamWriteHalf::Tls(tls_stream) => Pin::new(tls_stream).poll_flush(cx),
        }
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Error>> {
        match self.get_mut() {
            MaybeTlsStreamWriteHalf::Plain(stream) => Pin::new(stream).poll_shutdown(cx),
            MaybeTlsStreamWriteHalf::Tls(tls_stream) => Pin::new(tls_stream).poll_shutdown(cx),
        }
    }

    fn poll_write_vectored(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<Result<usize, Error>> {
        match self.get_mut() {
            MaybeTlsStreamWriteHalf::Plain(stream) => {
                Pin::new(stream).poll_write_vectored(cx, bufs)
            }
            MaybeTlsStreamWriteHalf::Tls(tls_stream) => {
                Pin::new(tls_stream).poll_write_vectored(cx, bufs)
            }
        }
    }

    fn is_write_vectored(&self) -> bool {
        match self {
            MaybeTlsStreamWriteHalf::Plain(stream) => stream.is_write_vectored(),
            MaybeTlsStreamWriteHalf::Tls(tls_stream) => tls_stream.is_write_vectored(),
        }
    }
}

#[allow(clippy::large_enum_variant)]
pub enum MaybeTlsStream<S> {
    Plain(S),
    Tls(client::TlsStream<S>),
}

impl<S> AsyncStream for MaybeTlsStream<S>
where
    S: AsyncStream + AsyncRead + AsyncWrite + Unpin,
{
    type R = MaybeTlsStreamReadHalf<S>;
    type W = MaybeTlsStreamWriteHalf<S>;

    fn into_split(self) -> (Self::R, Self::W) {
        match self {
            MaybeTlsStream::Plain(stream) => {
                let (r, w) = stream.into_split();
                (
                    MaybeTlsStreamReadHalf::Plain(r),
                    MaybeTlsStreamWriteHalf::Plain(w),
                )
            }
            MaybeTlsStream::Tls(tls_stream) => {
                let (r, w) = tls_stream.into_split();
                (
                    MaybeTlsStreamReadHalf::Tls(r),
                    MaybeTlsStreamWriteHalf::Tls(w),
                )
            }
        }
    }
}
