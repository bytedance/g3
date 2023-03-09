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

    async fn send_request_header<'a>(
        &'a mut self,
        req: &'a HttpProxyClientRequest,
    ) -> io::Result<()>;
}

#[async_trait]
pub(crate) trait HttpForwardRead: AsyncBufRead {
    fn update_stats(
        &mut self,
        task_stats: &ArcHttpForwardTaskRemoteStats,
        user_stats: Vec<Arc<UserUpstreamTrafficStats>>,
    );

    async fn recv_response_header<'a>(
        &'a mut self,
        method: &Method,
        keep_alive: bool,
        max_header_size: usize,
        http_notes: &'a mut HttpForwardTaskNotes,
    ) -> Result<HttpForwardRemoteResponse, HttpResponseParseError>;
}

pub(crate) struct HttpForwardWriterForAdaptation<'a> {
    pub(crate) inner: &'a mut BoxHttpForwardWriter,
}

impl<'a> AsyncWrite for HttpForwardWriterForAdaptation<'a> {
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

#[async_trait]
impl<'a> HttpRequestUpstreamWriter<HttpProxyClientRequest> for HttpForwardWriterForAdaptation<'a> {
    async fn send_request_header(&mut self, req: &HttpProxyClientRequest) -> io::Result<()> {
        self.inner.send_request_header(req).await
    }
}
