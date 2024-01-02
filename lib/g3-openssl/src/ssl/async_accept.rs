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
use std::task::{ready, Context, Poll};

use openssl::error::ErrorStack;
use openssl::ssl::{self, ErrorCode, Ssl, SslVersion};
use tokio::io::{AsyncRead, AsyncWrite};

use super::{AsyncEnginePoller, SslIoWrapper, SslStream};

pub struct SslAcceptor<S> {
    inner: ssl::SslStream<SslIoWrapper<S>>,
    async_engine: Option<AsyncEnginePoller>,
}

impl<S: AsyncRead + AsyncWrite + Unpin> SslAcceptor<S> {
    #[cfg(not(ossl300))]
    pub fn new(ssl: Ssl, stream: S) -> Result<Self, ErrorStack> {
        let wrapper = SslIoWrapper::new(stream);
        let async_engine = AsyncEnginePoller::new(&ssl);

        ssl::SslStream::new(ssl, wrapper).map(|inner| SslAcceptor {
            inner,
            async_engine,
        })
    }

    #[cfg(ossl300)]
    pub fn new(ssl: Ssl, stream: S) -> Result<Self, ErrorStack> {
        let wrapper = SslIoWrapper::new(stream);
        let async_engine = AsyncEnginePoller::new(&ssl)?;

        ssl::SslStream::new(ssl, wrapper).map(|inner| SslAcceptor {
            inner,
            async_engine,
        })
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> SslAcceptor<S> {
    pub fn poll_accept(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.inner.get_mut().set_cx(cx);
        #[cfg(ossl300)]
        if let Some(async_engine) = &self.async_engine {
            async_engine.set_cx(cx);
        }

        loop {
            match self.inner.accept() {
                Ok(_) => return Poll::Ready(Ok(())),
                Err(e) => match e.code() {
                    ErrorCode::WANT_READ | ErrorCode::WANT_WRITE => return Poll::Pending,
                    ErrorCode::WANT_ASYNC => {
                        if let Some(async_engine) = &mut self.async_engine {
                            ready!(async_engine.poll_ready(self.inner.ssl(), cx))?
                        } else {
                            return Poll::Ready(Err(io::Error::other(
                                "async engine poller is not set",
                            )));
                        }
                    }
                    ErrorCode::WANT_ASYNC_JOB => {
                        cx.waker().wake_by_ref();
                        return Poll::Pending;
                    }
                    _ => {
                        return Poll::Ready(Err(e.into_io_error().unwrap_or_else(io::Error::other)))
                    }
                },
            }
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
        Ok(SslStream::new(self.inner, self.async_engine))
    }
}
