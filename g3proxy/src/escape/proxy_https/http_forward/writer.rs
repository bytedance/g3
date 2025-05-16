/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
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

use super::ProxyHttpsEscaperConfig;
use crate::auth::UserUpstreamTrafficStats;
use crate::module::http_forward::{
    ArcHttpForwardTaskRemoteStats, HttpForwardTaskRemoteWrapperStats, HttpForwardWrite,
    send_req_header_to_origin, send_req_header_via_proxy,
};
use crate::serve::ServerTaskNotes;

pin_project! {
    pub(super) struct ProxyHttpsHttpForwardWriter<W: AsyncWrite> {
        config: Arc<ProxyHttpsEscaperConfig>,
        #[pin]
        inner: W,
        upstream: UpstreamAddr,
        pass_userid: Option<Arc<str>>,
    }
}

impl<W> ProxyHttpsHttpForwardWriter<W>
where
    W: AsyncWrite,
{
    pub(super) fn new(
        ups_w: W,
        config: &Arc<ProxyHttpsEscaperConfig>,
        upstream: UpstreamAddr,
    ) -> Self {
        ProxyHttpsHttpForwardWriter {
            config: Arc::clone(config),
            inner: ups_w,
            upstream,
            pass_userid: None,
        }
    }
}

impl<W> AsyncWrite for ProxyHttpsHttpForwardWriter<W>
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
impl<W> HttpForwardWrite for ProxyHttpsHttpForwardWriter<LimitedWriter<W>>
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
        let mut wrapper_stats = HttpForwardTaskRemoteWrapperStats::new(Arc::clone(task_stats));
        wrapper_stats.push_user_io_stats(user_stats);
        self.inner.reset_stats(Arc::new(wrapper_stats));
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
    pub(super) struct ProxyHttpsHttpRequestWriter<W: AsyncWrite> {
        config: Arc<ProxyHttpsEscaperConfig>,
        #[pin]
        inner: W,
    }
}

impl<W> ProxyHttpsHttpRequestWriter<W>
where
    W: AsyncWrite,
{
    pub(super) fn new(ups_w: W, config: &Arc<ProxyHttpsEscaperConfig>) -> Self {
        ProxyHttpsHttpRequestWriter {
            config: Arc::clone(config),
            inner: ups_w,
        }
    }
}

impl<W> AsyncWrite for ProxyHttpsHttpRequestWriter<W>
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
impl<W> HttpForwardWrite for ProxyHttpsHttpRequestWriter<LimitedWriter<W>>
where
    W: AsyncWrite + Send + Unpin,
{
    fn prepare_new(&mut self, _task_notes: &ServerTaskNotes, _upstream: &UpstreamAddr) {}

    fn update_stats(
        &mut self,
        task_stats: &ArcHttpForwardTaskRemoteStats,
        user_stats: Vec<Arc<UserUpstreamTrafficStats>>,
    ) {
        let mut wrapper_stats = HttpForwardTaskRemoteWrapperStats::new(Arc::clone(task_stats));
        wrapper_stats.push_user_io_stats(user_stats);
        self.inner.reset_stats(Arc::new(wrapper_stats));
    }

    async fn send_request_header(
        &mut self,
        req: &HttpProxyClientRequest,
        body: Option<&[u8]>,
    ) -> io::Result<()> {
        send_req_header_to_origin(&mut self.inner, req, body).await
    }
}
