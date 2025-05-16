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
use tokio::time::Instant;

use g3_http::server::HttpProxyClientRequest;
use g3_io_ext::LimitedWriter;
use g3_types::net::UpstreamAddr;

use crate::auth::UserUpstreamTrafficStats;
use crate::escape::proxy_float::peer::http::ProxyFloatHttpPeerSharedConfig;
use crate::module::http_forward::{
    ArcHttpForwardTaskRemoteStats, HttpForwardTaskRemoteWrapperStats, HttpForwardWrite,
    send_req_header_to_origin, send_req_header_via_proxy,
};
use crate::serve::ServerTaskNotes;

pin_project! {
    pub(super) struct HttpsPeerHttpForwardWriter<W: AsyncWrite> {
        config: Arc<ProxyFloatHttpPeerSharedConfig>,
        #[pin]
        inner: W,
        upstream: UpstreamAddr,
    }
}

impl<W> HttpsPeerHttpForwardWriter<W>
where
    W: AsyncWrite,
{
    pub(super) fn new(
        ups_w: W,
        config: &Arc<ProxyFloatHttpPeerSharedConfig>,
        upstream: UpstreamAddr,
    ) -> Self {
        HttpsPeerHttpForwardWriter {
            config: Arc::clone(config),
            inner: ups_w,
            upstream,
        }
    }
}

impl<W> AsyncWrite for HttpsPeerHttpForwardWriter<W>
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
impl<W> HttpForwardWrite for HttpsPeerHttpForwardWriter<LimitedWriter<W>>
where
    W: AsyncWrite + Send + Unpin,
{
    fn prepare_new(&mut self, _task_notes: &ServerTaskNotes, upstream: &UpstreamAddr) {
        self.upstream = upstream.clone();
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
        if let Some(expire) = &self.config.expire_instant {
            let now = Instant::now();
            if expire.checked_duration_since(now).is_none() {
                return Err(io::Error::other("connection has expired"));
            }
        }
        send_req_header_via_proxy(
            &mut self.inner,
            req,
            body,
            &self.upstream,
            &self.config.append_http_headers,
            None,
        )
        .await
    }
}

pin_project! {
    pub(super) struct HttpsPeerHttpRequestWriter<W: AsyncWrite> {
        config: Arc<ProxyFloatHttpPeerSharedConfig>,
        #[pin]
        inner: W,
    }
}

impl<W> HttpsPeerHttpRequestWriter<W>
where
    W: AsyncWrite,
{
    pub(super) fn new(ups_w: W, config: &Arc<ProxyFloatHttpPeerSharedConfig>) -> Self {
        HttpsPeerHttpRequestWriter {
            config: Arc::clone(config),
            inner: ups_w,
        }
    }
}

impl<W> AsyncWrite for HttpsPeerHttpRequestWriter<W>
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
impl<W> HttpForwardWrite for HttpsPeerHttpRequestWriter<LimitedWriter<W>>
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
        if let Some(expire) = &self.config.expire_instant {
            let now = Instant::now();
            if expire.checked_duration_since(now).is_none() {
                return Err(io::Error::other("connection has expired"));
            }
        }
        send_req_header_to_origin(&mut self.inner, req, body).await
    }
}
