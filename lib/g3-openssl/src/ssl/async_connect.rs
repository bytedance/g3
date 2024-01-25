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
use openssl::ssl::{self, ErrorCode, Ssl};
use tokio::io::{AsyncRead, AsyncWrite};

use super::{AsyncEnginePoller, SslIoWrapper, SslStream};

pub struct SslConnector<S> {
    inner: ssl::SslStream<SslIoWrapper<S>>,
    async_engine: Option<AsyncEnginePoller>,
}

impl<S: AsyncRead + AsyncWrite + Unpin> SslConnector<S> {
    pub fn new(ssl: Ssl, stream: S) -> Result<Self, ErrorStack> {
        let wrapper = SslIoWrapper::new(stream);
        let async_engine = AsyncEnginePoller::new(&ssl)?;

        ssl::SslStream::new(ssl, wrapper).map(|inner| SslConnector {
            inner,
            async_engine,
        })
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> SslConnector<S> {
    pub fn poll_connect(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.inner.get_mut().set_cx(cx);
        #[cfg(ossl300)]
        if let Some(async_engine) = &self.async_engine {
            async_engine.set_cx(cx);
        }

        loop {
            match self.inner.connect() {
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

    pub async fn connect(mut self) -> io::Result<SslStream<S>> {
        future::poll_fn(|cx| self.poll_connect(cx)).await?;
        Ok(SslStream::new(self.inner, None))
    }
}
