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

use std::io;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use async_trait::async_trait;
use http::Method;
use pin_project::pin_project;
use tokio::io::{AsyncBufRead, AsyncRead, ReadBuf};

use g3_http::client::{HttpForwardRemoteResponse, HttpResponseParseError};
use g3_io_ext::LimitedBufReader;

use crate::auth::UserUpstreamTrafficStats;
use crate::module::http_forward::{
    ArcHttpForwardTaskRemoteStats, HttpForwardRead, HttpForwardRemoteStatsWrapper,
    HttpForwardTaskNotes,
};

#[pin_project]
pub(super) struct ProxyHttpHttpForwardReader<R: AsyncRead> {
    #[pin]
    inner: LimitedBufReader<R>,
}

impl<R> ProxyHttpHttpForwardReader<R>
where
    R: AsyncRead + Unpin,
{
    pub(super) fn new(ups_r: LimitedBufReader<R>) -> Self {
        ProxyHttpHttpForwardReader { inner: ups_r }
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

impl<R> AsyncRead for ProxyHttpHttpForwardReader<R>
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

impl<R> AsyncBufRead for ProxyHttpHttpForwardReader<R>
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
impl<R> HttpForwardRead for ProxyHttpHttpForwardReader<R>
where
    R: AsyncRead + Send + Unpin,
{
    fn update_stats(
        &mut self,
        task_stats: &ArcHttpForwardTaskRemoteStats,
        user_stats: Vec<Arc<UserUpstreamTrafficStats>>,
    ) {
        let mut wrapper_stats = HttpForwardRemoteStatsWrapper::new(Arc::clone(task_stats));
        wrapper_stats.push_user_io_stats(user_stats);
        self.inner.reset_buffer_stats(wrapper_stats.into_reader());
    }

    async fn recv_response_header<'a>(
        &'a mut self,
        method: &Method,
        keep_alive: bool,
        max_header_size: usize,
        http_notes: &'a mut HttpForwardTaskNotes,
    ) -> Result<HttpForwardRemoteResponse, HttpResponseParseError> {
        self.get_rsp_header(method, keep_alive, max_header_size, http_notes)
            .await
    }
}
