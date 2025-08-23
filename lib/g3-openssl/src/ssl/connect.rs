/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::future;
use std::io;

use std::task::{Context, Poll};

use openssl::error::ErrorStack;
use openssl::ssl::{self, ErrorCode, Ssl};
use tokio::io::{AsyncRead, AsyncWrite};

use super::{ConvertSslError, SslErrorAction, SslIoWrapper, SslStream};

pub struct SslConnector<S> {
    inner: ssl::SslStream<SslIoWrapper<S>>,
}

impl<S: AsyncRead + AsyncWrite + Unpin> SslConnector<S> {
    pub fn new(ssl: Ssl, stream: S) -> Result<Self, ErrorStack> {
        ssl::SslStream::new(ssl, SslIoWrapper::new(stream)).map(|inner| SslConnector { inner })
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> SslConnector<S> {
    pub fn poll_connect(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.inner.get_mut().set_cx(cx);

        match self.inner.connect() {
            Ok(_) => Poll::Ready(Ok(())),
            Err(e) => match e.code() {
                ErrorCode::WANT_READ | ErrorCode::WANT_WRITE => Poll::Pending,
                _ => Poll::Ready(Err(e
                    .into_io_error()
                    .unwrap_or_else(|e| e.build_io_error(SslErrorAction::Connect)))),
            },
        }
    }

    pub async fn connect(mut self) -> io::Result<SslStream<S>> {
        future::poll_fn(|cx| self.poll_connect(cx)).await?;
        Ok(SslStream::new(self.inner))
    }
}
