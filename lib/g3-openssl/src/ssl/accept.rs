/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::future::poll_fn;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll};
use std::time::Duration;

use openssl::error::ErrorStack;
use openssl::ssl::{self, ErrorCode, Ssl};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::time::Sleep;

use super::{ConvertSslError, SslErrorAction, SslIoWrapper, SslStream};

pub struct SslAcceptor<S> {
    inner: ssl::SslStream<SslIoWrapper<S>>,
    sleep_future: Pin<Box<Sleep>>,
}

impl<S: AsyncRead + AsyncWrite + Unpin> SslAcceptor<S> {
    pub fn new(ssl: Ssl, stream: S, timeout: Duration) -> Result<Self, ErrorStack> {
        let sleep_future = tokio::time::sleep(timeout);
        ssl::SslStream::new(ssl, SslIoWrapper::new(stream)).map(|inner| SslAcceptor {
            inner,
            sleep_future: Box::pin(sleep_future),
        })
    }

    #[cfg(not(libressl))]
    pub(crate) fn with_inner(
        inner: ssl::SslStream<SslIoWrapper<S>>,
        timeout: Duration,
    ) -> Result<Self, ErrorStack> {
        let sleep_future = tokio::time::sleep(timeout);
        Ok(SslAcceptor {
            inner,
            sleep_future: Box::pin(sleep_future),
        })
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> SslAcceptor<S> {
    pub fn poll_accept(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        match Pin::new(&mut self.sleep_future).poll(cx) {
            Poll::Pending => {}
            Poll::Ready(_) => {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "ssl accept timed out",
                )));
            }
        }

        self.inner.get_mut().set_cx(cx);

        match self.inner.accept() {
            Ok(_) => Poll::Ready(Ok(())),
            Err(e) => match e.code() {
                ErrorCode::WANT_READ | ErrorCode::WANT_WRITE => Poll::Pending,
                _ => Poll::Ready(Err(e
                    .into_io_error()
                    .unwrap_or_else(|e| e.build_io_error(SslErrorAction::Accept)))),
            },
        }
    }

    pub async fn accept(mut self) -> io::Result<SslStream<S>> {
        poll_fn(|cx| self.poll_accept(cx)).await?;
        Ok(SslStream::new(self.inner))
    }
}
