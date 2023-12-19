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
use std::task::{Context, Poll};

use openssl::error::ErrorStack;
use openssl::ssl::{self, Ssl};
use openssl_sys::{SSL_ERROR_WANT_READ, SSL_ERROR_WANT_WRITE};
use tokio::io::{AsyncRead, AsyncWrite};

use super::error::{SSL_ERROR_WANT_ASYNC, SSL_ERROR_WANT_ASYNC_JOB};
use super::{SslIoWrapper, SslStream};

pub struct SslConnector<S> {
    inner: ssl::SslStream<SslIoWrapper<S>>,
}

impl<S: AsyncRead + AsyncWrite + Unpin> SslConnector<S> {
    pub fn new(ssl: Ssl, stream: S) -> Result<Self, ErrorStack> {
        let wrapper = SslIoWrapper::new(stream);
        ssl::SslStream::new(ssl, wrapper).map(|inner| SslConnector { inner })
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> SslConnector<S> {
    pub fn poll_connect(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), ssl::Error>> {
        self.inner.get_mut().set_cx(cx);

        match self.inner.connect() {
            Ok(_) => Poll::Ready(Ok(())),
            Err(e) => match e.code().as_raw() {
                SSL_ERROR_WANT_READ | SSL_ERROR_WANT_WRITE => Poll::Pending,
                SSL_ERROR_WANT_ASYNC => {
                    // TODO
                    todo!()
                }
                SSL_ERROR_WANT_ASYNC_JOB => {
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
                _ => Poll::Ready(Err(e)),
            },
        }
    }

    pub async fn connect(mut self) -> Result<SslStream<S>, ssl::Error> {
        future::poll_fn(|cx| self.poll_connect(cx)).await?;
        Ok(SslStream::new(self.inner))
    }
}
