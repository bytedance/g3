/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::future::poll_fn;
use std::io;
use std::pin::Pin;
use std::task::{Context, Poll, ready};
use std::time::Duration;

use openssl::error::ErrorStack;
use openssl::ssl::{self, ErrorCode, Ssl};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::time::Sleep;

use super::{
    AsyncEnginePoller, ConvertSslError, SslAsyncModeExt, SslErrorAction, SslIoWrapper, SslStream,
};

pub struct SslAcceptor<S> {
    inner: ssl::SslStream<SslIoWrapper<S>>,
    async_engine: Option<AsyncEnginePoller>,
    sleep_future: Pin<Box<Sleep>>,
    wait_async_job: bool,
}

impl<S: AsyncRead + AsyncWrite + Unpin> SslAcceptor<S> {
    pub fn new(ssl: Ssl, stream: S, timeout: Duration) -> Result<Self, ErrorStack> {
        let wrapper = SslIoWrapper::new(stream);
        let async_engine = AsyncEnginePoller::new(&ssl)?;
        let sleep_future = tokio::time::sleep(timeout);

        ssl::SslStream::new(ssl, wrapper).map(|inner| SslAcceptor {
            inner,
            async_engine,
            sleep_future: Box::pin(sleep_future),
            wait_async_job: false,
        })
    }

    pub(crate) fn with_inner(
        inner: ssl::SslStream<SslIoWrapper<S>>,
        timeout: Duration,
    ) -> Result<Self, ErrorStack> {
        let async_engine = AsyncEnginePoller::new(inner.ssl())?;
        let sleep_future = tokio::time::sleep(timeout);
        Ok(SslAcceptor {
            inner,
            async_engine,
            sleep_future: Box::pin(sleep_future),
            wait_async_job: false,
        })
    }
}

impl<S: AsyncRead + AsyncWrite + Unpin> SslAcceptor<S> {
    fn poll_wait_async_job(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let Some(async_engine) = &mut self.async_engine else {
            return Poll::Ready(Ok(()));
        };

        if !self.inner.ssl().waiting_for_async() {
            return Poll::Ready(Ok(()));
        }

        if !async_engine.is_poll_pending() {
            return Poll::Ready(Ok(()));
        }

        match self.inner.accept() {
            Ok(_) => Poll::Ready(Ok(())),
            Err(e) => match e.code() {
                ErrorCode::WANT_READ | ErrorCode::WANT_WRITE => Poll::Pending,
                ErrorCode::WANT_ASYNC => async_engine.poll_ready(self.inner.ssl(), cx),
                ErrorCode::WANT_ASYNC_JOB => Poll::Ready(Ok(())),
                _ => Poll::Ready(Err(e
                    .into_io_error()
                    .unwrap_or_else(|e| e.build_io_error(SslErrorAction::Accept)))),
            },
        }
    }

    fn poll_accept(&mut self, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        self.inner.get_mut().set_cx(cx);
        #[cfg(ossl300)]
        if let Some(async_engine) = &self.async_engine {
            async_engine.set_cx(cx);
        }

        loop {
            if self.wait_async_job {
                ready!(self.poll_wait_async_job(cx))?;
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "ssl accept timed out",
                )));
            }

            match Pin::new(&mut self.sleep_future).poll(cx) {
                Poll::Pending => break,
                Poll::Ready(_) => self.wait_async_job = true,
            }
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
                        return Poll::Ready(Err(e
                            .into_io_error()
                            .unwrap_or_else(|e| e.build_io_error(SslErrorAction::Accept))));
                    }
                },
            }
        }
    }

    /// Accept ssl handshake and get a connected SSL Stream
    ///
    /// # Cancellation
    ///
    /// Not supported. Users have to wait for the future to run to the end, or you may encounter
    /// crashes in OpenSSL. See:
    ///   - https://github.com/intel/QAT_Engine/issues/292
    ///   - https://github.com/openssl/openssl/discussions/23158
    pub async fn accept(mut self) -> io::Result<SslStream<S>> {
        poll_fn(|cx| self.poll_accept(cx)).await?;
        Ok(SslStream::new(self.inner, self.async_engine))
    }
}
