/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io::{self, IoSlice};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use async_trait::async_trait;
use http::Method;
use tokio::io::{AsyncBufRead, AsyncWrite};

use g3_http::client::{HttpForwardRemoteResponse, HttpResponseParseError};
use g3_http::server::HttpProxyClientRequest;
use g3_icap_client::reqmod::h1::HttpRequestUpstreamWriter;
use g3_types::net::UpstreamAddr;

use super::{ArcHttpForwardTaskRemoteStats, HttpForwardTaskNotes};
use crate::auth::UserUpstreamTrafficStats;
use crate::serve::ServerTaskNotes;

mod writer;
pub(crate) use writer::{send_req_header_to_origin, send_req_header_via_proxy};

mod eof_poller;
pub(crate) use eof_poller::HttpConnectionEofPoller;

pub(crate) type BoxHttpForwardWriter = Box<dyn HttpForwardWrite + Send + Unpin>;
pub(crate) type BoxHttpForwardReader = Box<dyn HttpForwardRead + Send + Unpin>;
pub(crate) type BoxHttpForwardConnection = (BoxHttpForwardWriter, BoxHttpForwardReader);

#[async_trait]
pub(crate) trait HttpForwardWrite: AsyncWrite {
    fn prepare_new(&mut self, task_notes: &ServerTaskNotes, upstream: &UpstreamAddr);
    fn update_stats(
        &mut self,
        task_stats: &ArcHttpForwardTaskRemoteStats,
        user_stats: Vec<Arc<UserUpstreamTrafficStats>>,
    );

    async fn send_request_header(
        &mut self,
        req: &HttpProxyClientRequest,
        body: Option<&[u8]>,
    ) -> io::Result<()>;
}

#[async_trait]
pub(crate) trait HttpForwardRead: AsyncBufRead {
    fn update_stats(
        &mut self,
        task_stats: &ArcHttpForwardTaskRemoteStats,
        user_stats: Vec<Arc<UserUpstreamTrafficStats>>,
    );

    async fn recv_response_header(
        &mut self,
        method: &Method,
        keep_alive: bool,
        max_header_size: usize,
        http_notes: &mut HttpForwardTaskNotes,
    ) -> Result<HttpForwardRemoteResponse, HttpResponseParseError>;
}

pub(crate) struct HttpForwardWriterForAdaptation<'a> {
    pub(crate) inner: &'a mut BoxHttpForwardWriter,
}

impl AsyncWrite for HttpForwardWriterForAdaptation<'_> {
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

impl HttpRequestUpstreamWriter<HttpProxyClientRequest> for HttpForwardWriterForAdaptation<'_> {
    async fn send_request_header(&mut self, req: &HttpProxyClientRequest) -> io::Result<()> {
        self.inner.send_request_header(req, None).await
    }
}
