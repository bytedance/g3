/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use async_trait::async_trait;
use http::Method;
use pin_project_lite::pin_project;
use tokio::io::{AsyncBufRead, AsyncRead, ReadBuf};

use g3_http::client::{HttpForwardRemoteResponse, HttpResponseParseError};
use g3_io_ext::LimitedBufReader;

use crate::auth::UserUpstreamTrafficStats;
use crate::module::http_forward::{
    ArcHttpForwardTaskRemoteStats, HttpForwardRead, HttpForwardTaskNotes,
    HttpForwardTaskRemoteWrapperStats,
};

pin_project! {
    pub(super) struct ProxyHttpsHttpForwardReader<R: AsyncRead> {
        #[pin]
        inner: LimitedBufReader<R>,
    }
}

impl<R> ProxyHttpsHttpForwardReader<R>
where
    R: AsyncRead + Unpin,
{
    pub(super) fn new(ups_r: LimitedBufReader<R>) -> Self {
        ProxyHttpsHttpForwardReader { inner: ups_r }
    }

    async fn get_rsp_header(
        &mut self,
        method: &Method,
        keep_alive: bool,
        max_header_size: usize,
        http_notes: &mut HttpForwardTaskNotes,
    ) -> Result<HttpForwardRemoteResponse, HttpResponseParseError> {
        let rsp =
            HttpForwardRemoteResponse::parse(&mut self.inner, method, keep_alive, max_header_size)
                .await?;
        http_notes.rsp_status = rsp.code;
        http_notes.origin_status = rsp.code;
        // TODO detect and set outgoing_addr for and target_addr supported remote proxies except for g3proxy
        Ok(rsp)
    }
}

impl<R> AsyncRead for ProxyHttpsHttpForwardReader<R>
where
    R: AsyncRead,
{
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut ReadBuf<'_>,
    ) -> Poll<io::Result<()>> {
        let this = self.project();
        this.inner.poll_read(cx, buf)
    }
}

impl<R> AsyncBufRead for ProxyHttpsHttpForwardReader<R>
where
    R: AsyncRead,
{
    fn poll_fill_buf(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<&[u8]>> {
        let this = self.project();
        this.inner.poll_fill_buf(cx)
    }

    fn consume(self: Pin<&mut Self>, amt: usize) {
        let this = self.project();
        this.inner.consume(amt)
    }
}

#[async_trait]
impl<R> HttpForwardRead for ProxyHttpsHttpForwardReader<R>
where
    R: AsyncRead + Send + Unpin,
{
    fn update_stats(
        &mut self,
        task_stats: &ArcHttpForwardTaskRemoteStats,
        user_stats: Vec<Arc<UserUpstreamTrafficStats>>,
    ) {
        let mut wrapper_stats = HttpForwardTaskRemoteWrapperStats::new(Arc::clone(task_stats));
        wrapper_stats.push_user_io_stats(user_stats);
        self.inner.reset_buffer_stats(Arc::new(wrapper_stats));
    }

    async fn recv_response_header(
        &mut self,
        method: &Method,
        keep_alive: bool,
        max_header_size: usize,
        http_notes: &mut HttpForwardTaskNotes,
    ) -> Result<HttpForwardRemoteResponse, HttpResponseParseError> {
        self.get_rsp_header(method, keep_alive, max_header_size, http_notes)
            .await
    }
}
