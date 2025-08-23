/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2024-2025 ByteDance and/or its affiliates.
 */

use tokio::io::{AsyncRead, AsyncWrite, ReadHalf, WriteHalf};

use g3_openssl::SslStream;

use super::AsyncStream;

impl<S> AsyncStream for SslStream<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    type R = ReadHalf<SslStream<S>>;
    type W = WriteHalf<SslStream<S>>;

    fn into_split(self) -> (Self::R, Self::W) {
        tokio::io::split(self)
    }
}

pub enum MaybeSslStreamReadHalf<S: AsyncStream> {
    Plain(S::R),
    Ssl(ReadHalf<SslStream<S>>),
}

pub enum MaybeSslStreamWriteHalf<S: AsyncStream> {
    Plain(S::W),
    Ssl(WriteHalf<SslStream<S>>),
}

pub enum MaybeSslStream<S> {
    Plain(S),
    Ssl(SslStream<S>),
}

impl<S> AsyncStream for MaybeSslStream<S>
where
    S: AsyncStream + AsyncRead + AsyncWrite + Unpin,
{
    type R = MaybeSslStreamReadHalf<S>;
    type W = MaybeSslStreamWriteHalf<S>;

    fn into_split(self) -> (Self::R, Self::W) {
        match self {
            MaybeSslStream::Plain(stream) => {
                let (r, w) = stream.into_split();
                (
                    MaybeSslStreamReadHalf::Plain(r),
                    MaybeSslStreamWriteHalf::Plain(w),
                )
            }
            MaybeSslStream::Ssl(ssl_stream) => {
                let (r, w) = ssl_stream.into_split();
                (
                    MaybeSslStreamReadHalf::Ssl(r),
                    MaybeSslStreamWriteHalf::Ssl(w),
                )
            }
        }
    }
}
