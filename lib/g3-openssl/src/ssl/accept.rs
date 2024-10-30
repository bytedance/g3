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

use std::future;
use std::io;
use std::task::{Context, Poll};

use openssl::error::ErrorStack;
use openssl::ssl::{self, ErrorCode, Ssl};
use tokio::io::{AsyncRead, AsyncWrite};

use super::{SslIoWrapper, SslStream};

pub struct SslAcceptor<S> {
    pub(crate) inner: ssl::SslStream<SslIoWrapper<S>>,
}

impl<S: AsyncRead + AsyncWrite + Unpin> SslAcceptor<S> {
    pub fn new(ssl: Ssl, stream: S) -> Result<Self, ErrorStack> {
        ssl::SslStream::new(ssl, SslIoWrapper::new(stream)).map(|inner| SslAcceptor { inner })
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> SslAcceptor<S> {
    pub fn poll_accept(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.inner.get_mut().set_cx(cx);

        match self.inner.accept() {
            Ok(_) => Poll::Ready(Ok(())),
            Err(e) => match e.code() {
                ErrorCode::WANT_READ | ErrorCode::WANT_WRITE => Poll::Pending,
                _ => Poll::Ready(Err(e.into_io_error().unwrap_or_else(io::Error::other))),
            },
        }
    }

    pub async fn accept(mut self) -> io::Result<SslStream<S>> {
        future::poll_fn(|cx| self.poll_accept(cx)).await?;
        let ssl = self.inner.ssl();
        if let Some(session) = ssl.session() {
            if session.protocol_version() == SslVersion::TLS1_3 && ssl.session_reused() {
                // do session resumption only once according to TLS1.3
                unsafe { ssl.ssl_context().remove_session(session) };
            }
        }
        Ok(SslStream::new(self.inner))
    }
}
