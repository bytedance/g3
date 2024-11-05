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

use std::future;
use std::io;
use std::task::{Context, Poll};
use std::time::Duration;

use openssl::error::ErrorStack;
use openssl::ssl::{self, ErrorCode, Ssl, SslRef};
use tokio::io::{AsyncRead, AsyncWrite};

use super::{SslAcceptor, SslIoWrapper};

pub struct SslLazyAcceptor<S> {
    inner: ssl::SslStream<SslIoWrapper<S>>,
}

impl<S: AsyncRead + AsyncWrite + Unpin> SslLazyAcceptor<S> {
    pub fn new(ssl: Ssl, stream: S) -> Result<Self, ErrorStack> {
        ssl::SslStream::new(ssl, SslIoWrapper::new(stream)).map(|inner| SslLazyAcceptor { inner })
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> SslLazyAcceptor<S> {
    pub fn poll_accept(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.inner.get_mut().set_cx(cx);

        match self.inner.accept() {
            Ok(_) => Poll::Ready(Ok(())),
            Err(e) => match e.code() {
                ErrorCode::WANT_READ | ErrorCode::WANT_WRITE => Poll::Pending,
                #[cfg(not(any(feature = "aws-lc", feature = "boringssl")))]
                ErrorCode::WANT_CLIENT_HELLO_CB => Poll::Ready(Ok(())),
                #[cfg(any(feature = "aws-lc", feature = "boringssl"))]
                ErrorCode::PENDING_CERTIFICATE => Poll::Ready(Ok(())),
                _ => Poll::Ready(Err(e.into_io_error().unwrap_or_else(io::Error::other))),
            },
        }
    }

    pub async fn accept(&mut self) -> io::Result<()> {
        future::poll_fn(|cx| self.poll_accept(cx)).await
    }

    pub fn into_acceptor(self, timeout: Duration) -> Result<SslAcceptor<S>, ErrorStack> {
        SslAcceptor::with_inner(self.inner, timeout)
    }

    pub fn ssl(&self) -> &SslRef {
        self.inner.ssl()
    }

    pub fn ssl_mut(&mut self) -> &mut SslRef {
        self.inner.ssl_mut()
    }
}
