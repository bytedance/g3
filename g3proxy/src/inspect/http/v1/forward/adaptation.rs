/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io::{self, IoSlice};
use std::pin::Pin;
use std::task::{Context, Poll};

use tokio::io::{AsyncWrite, AsyncWriteExt};

use g3_http::server::HttpTransparentRequest;
use g3_icap_client::reqmod::h1::HttpRequestUpstreamWriter;

pub(crate) struct HttpRequestWriterForAdaptation<'a, W> {
    pub(crate) inner: &'a mut W,
}

impl<W> AsyncWrite for HttpRequestWriterForAdaptation<'_, W>
where
    W: AsyncWrite + Unpin,
{
    fn poll_write(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write(cx, buf)
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(cx)
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }

    fn poll_write_vectored(
        mut self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        bufs: &[IoSlice<'_>],
    ) -> Poll<io::Result<usize>> {
        Pin::new(&mut self.inner).poll_write_vectored(cx, bufs)
    }

    fn is_write_vectored(&self) -> bool {
        self.inner.is_write_vectored()
    }
}

impl<W> HttpRequestUpstreamWriter<HttpTransparentRequest> for HttpRequestWriterForAdaptation<'_, W>
where
    W: AsyncWrite + Send + Unpin,
{
    async fn send_request_header(&mut self, req: &HttpTransparentRequest) -> io::Result<()> {
        let head = req.serialize_for_origin();
        self.inner.write_all(&head).await
    }
}
