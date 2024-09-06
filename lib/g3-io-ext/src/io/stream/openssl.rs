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
