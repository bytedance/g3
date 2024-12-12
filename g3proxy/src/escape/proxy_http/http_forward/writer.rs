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
use pin_project_lite::pin_project;
use tokio::io::AsyncWrite;

use g3_http::server::HttpProxyClientRequest;
use g3_io_ext::LimitedWriter;
use g3_types::net::UpstreamAddr;

use super::{ProxyHttpEscaperConfig, ProxyHttpEscaperStats};
use crate::auth::UserUpstreamTrafficStats;
use crate::module::http_forward::{
    send_req_header_to_origin, send_req_header_via_proxy, ArcHttpForwardTaskRemoteStats,
    HttpForwardRemoteWrapperStats, HttpForwardTaskRemoteWrapperStats, HttpForwardWrite,
};
use crate::serve::ServerTaskNotes;

pin_project! {
    pub(super) struct ProxyHttpHttpForwardWriter<W: AsyncWrite> {
        config: Arc<ProxyHttpEscaperConfig>,
        #[pin]
        inner: W,
        escaper_stats: Option<Arc<ProxyHttpEscaperStats>>,
        upstream: UpstreamAddr,
        pass_userid: Option<Arc<str>>,
    }
}

impl<W> ProxyHttpHttpForwardWriter<W>
where
    W: AsyncWrite,
{
    pub(super) fn new(
        ups_w: W,
        escaper_stats: Option<Arc<ProxyHttpEscaperStats>>,
        config: &Arc<ProxyHttpEscaperConfig>,
        upstream: UpstreamAddr,
    ) -> Self {
        ProxyHttpHttpForwardWriter {
            config: Arc::clone(config),
            inner: ups_w,
            escaper_stats,
            upstream,
            pass_userid: None,
        }
    }
}

impl<W> AsyncWrite for ProxyHttpHttpForwardWriter<W>
where
    W: AsyncWrite,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.project();
        this.inner.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.project();
        this.inner.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.project();
        this.inner.poll_shutdown(cx)
    }
}

#[async_trait]
impl<W> HttpForwardWrite for ProxyHttpHttpForwardWriter<LimitedWriter<W>>
where
    W: AsyncWrite + Send + Unpin,
{
    fn prepare_new(&mut self, task_notes: &ServerTaskNotes, upstream: &UpstreamAddr) {
        self.upstream = upstream.clone();
        self.pass_userid = task_notes.raw_user_name().cloned();
    }

    fn update_stats(
        &mut self,
        task_stats: &ArcHttpForwardTaskRemoteStats,
        user_stats: Vec<Arc<UserUpstreamTrafficStats>>,
    ) {
        if let Some(escaper_stats) = &self.escaper_stats {
            let mut wrapper_stats = HttpForwardRemoteWrapperStats::new(escaper_stats, task_stats);
            wrapper_stats.push_user_io_stats(user_stats);
            self.inner.reset_stats(Arc::new(wrapper_stats));
        } else {
            let mut wrapper_stats = HttpForwardTaskRemoteWrapperStats::new(Arc::clone(task_stats));
            wrapper_stats.push_user_io_stats(user_stats);
            self.inner.reset_stats(Arc::new(wrapper_stats));
        }
    }

    async fn send_request_header(
        &mut self,
        req: &HttpProxyClientRequest,
        body: Option<&[u8]>,
    ) -> io::Result<()> {
        let userid = self.pass_userid.as_deref();
        send_req_header_via_proxy(
            &mut self.inner,
            req,
            body,
            &self.upstream,
            &self.config.append_http_headers,
            userid,
        )
        .await
    }
}

pin_project! {
    pub(super) struct ProxyHttpHttpRequestWriter<W: AsyncWrite> {
        config: Arc<ProxyHttpEscaperConfig>,
        #[pin]
        inner: W,
        escaper_stats: Option<Arc<ProxyHttpEscaperStats>>,
    }
}

impl<W> ProxyHttpHttpRequestWriter<W>
where
    W: AsyncWrite,
{
    pub(super) fn new(
        ups_w: W,
        escaper_stats: Option<Arc<ProxyHttpEscaperStats>>,
        config: &Arc<ProxyHttpEscaperConfig>,
    ) -> Self {
        ProxyHttpHttpRequestWriter {
            config: Arc::clone(config),
            inner: ups_w,
            escaper_stats,
        }
    }
}

impl<W> AsyncWrite for ProxyHttpHttpRequestWriter<W>
where
    W: AsyncWrite,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        let this = self.project();
        this.inner.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.project();
        this.inner.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let this = self.project();
        this.inner.poll_shutdown(cx)
    }
}

#[async_trait]
impl<W> HttpForwardWrite for ProxyHttpHttpRequestWriter<LimitedWriter<W>>
where
    W: AsyncWrite + Send + Unpin,
{
    fn prepare_new(&mut self, _task_notes: &ServerTaskNotes, _upstream: &UpstreamAddr) {}

    fn update_stats(
        &mut self,
        task_stats: &ArcHttpForwardTaskRemoteStats,
        user_stats: Vec<Arc<UserUpstreamTrafficStats>>,
    ) {
        if let Some(escaper_stats) = &self.escaper_stats {
            let mut wrapper_stats = HttpForwardRemoteWrapperStats::new(escaper_stats, task_stats);
            wrapper_stats.push_user_io_stats(user_stats);
            self.inner.reset_stats(Arc::new(wrapper_stats));
        } else {
            let mut wrapper_stats = HttpForwardTaskRemoteWrapperStats::new(Arc::clone(task_stats));
            wrapper_stats.push_user_io_stats(user_stats);
            self.inner.reset_stats(Arc::new(wrapper_stats));
        }
    }

    async fn send_request_header(
        &mut self,
        req: &HttpProxyClientRequest,
        body: Option<&[u8]>,
    ) -> io::Result<()> {
        send_req_header_to_origin(&mut self.inner, req, body).await
    }
}
