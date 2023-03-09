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
use pin_project::pin_project;
use tokio::io::AsyncWrite;
use tokio::time::Instant;

use g3_http::server::HttpProxyClientRequest;
use g3_io_ext::LimitedWriter;
use g3_types::net::UpstreamAddr;

use super::{ProxyFloatEscaperStats, ProxyFloatHttpPeerSharedConfig};
use crate::auth::UserUpstreamTrafficStats;
use crate::escape::proxy_float::stats::ProxyHttpMixedRemoteStats;
use crate::module::http_forward::{
    send_req_header_to_origin, send_req_header_via_proxy, ArcHttpForwardTaskRemoteStats,
    HttpForwardRemoteStatsWrapper, HttpForwardWrite,
};
use crate::serve::ServerTaskNotes;

#[pin_project]
pub(super) struct HttpPeerHttpForwardWriter<W: AsyncWrite> {
    config: Arc<ProxyFloatHttpPeerSharedConfig>,
    #[pin]
    inner: W,
    upstream: UpstreamAddr,
    escaper_stats: Option<Arc<ProxyFloatEscaperStats>>,
}

impl<W> HttpPeerHttpForwardWriter<W>
where
    W: AsyncWrite,
{
    pub(super) fn new(
        ups_w: W,
        escaper_stats: Option<Arc<ProxyFloatEscaperStats>>,
        config: &Arc<ProxyFloatHttpPeerSharedConfig>,
        upstream: UpstreamAddr,
    ) -> Self {
        HttpPeerHttpForwardWriter {
            config: Arc::clone(config),
            inner: ups_w,
            upstream,
            escaper_stats,
        }
    }
}

impl<W> AsyncWrite for HttpPeerHttpForwardWriter<W>
where
    W: AsyncWrite,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        let this = self.project();
        this.inner.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        let this = self.project();
        this.inner.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        let this = self.project();
        this.inner.poll_shutdown(cx)
    }
}

#[async_trait]
impl<W> HttpForwardWrite for HttpPeerHttpForwardWriter<LimitedWriter<W>>
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
        if let Some(escaper_stats) = &self.escaper_stats {
            let mut wrapper_stats = ProxyHttpMixedRemoteStats::new(escaper_stats, task_stats);
            wrapper_stats.push_user_io_stats(user_stats);
            self.inner.reset_stats(wrapper_stats.into_writer());
        } else {
            let mut wrapper_stats = HttpForwardRemoteStatsWrapper::new(Arc::clone(task_stats));
            wrapper_stats.push_user_io_stats(user_stats);
            self.inner.reset_stats(wrapper_stats.into_writer());
        }
    }

    async fn send_request_header<'a>(
        &'a mut self,
        req: &'a HttpProxyClientRequest,
    ) -> io::Result<()> {
        if let Some(expire) = &self.config.expire_instant {
            let now = Instant::now();
            if expire.checked_duration_since(now).is_none() {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "connection has expired",
                ));
            }
        }
        send_req_header_via_proxy(
            &mut self.inner,
            req,
            &self.upstream,
            &self.config.append_http_headers,
            None,
        )
        .await
    }
}

#[pin_project]
pub(super) struct HttpPeerHttpRequestWriter<W: AsyncWrite> {
    config: Arc<ProxyFloatHttpPeerSharedConfig>,
    #[pin]
    inner: W,
    escaper_stats: Option<Arc<ProxyFloatEscaperStats>>,
}

impl<W> HttpPeerHttpRequestWriter<W>
where
    W: AsyncWrite,
{
    pub(super) fn new(
        ups_w: W,
        escaper_stats: Option<Arc<ProxyFloatEscaperStats>>,
        config: &Arc<ProxyFloatHttpPeerSharedConfig>,
    ) -> Self {
        HttpPeerHttpRequestWriter {
            config: Arc::clone(config),
            inner: ups_w,
            escaper_stats,
        }
    }
}

impl<W> AsyncWrite for HttpPeerHttpRequestWriter<W>
where
    W: AsyncWrite,
{
    fn poll_write(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<Result<usize, io::Error>> {
        let this = self.project();
        this.inner.poll_write(cx, buf)
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        let this = self.project();
        this.inner.poll_flush(cx)
    }

    fn poll_shutdown(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), io::Error>> {
        let this = self.project();
        this.inner.poll_shutdown(cx)
    }
}

#[async_trait]
impl<W> HttpForwardWrite for HttpPeerHttpRequestWriter<LimitedWriter<W>>
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
            let mut wrapper_stats = ProxyHttpMixedRemoteStats::new(escaper_stats, task_stats);
            wrapper_stats.push_user_io_stats(user_stats);
            self.inner.reset_stats(wrapper_stats.into_writer());
        } else {
            let mut wrapper_stats = HttpForwardRemoteStatsWrapper::new(Arc::clone(task_stats));
            wrapper_stats.push_user_io_stats(user_stats);
            self.inner.reset_stats(wrapper_stats.into_writer());
        }
    }

    async fn send_request_header<'a>(
        &'a mut self,
        req: &'a HttpProxyClientRequest,
    ) -> io::Result<()> {
        if let Some(expire) = &self.config.expire_instant {
            let now = Instant::now();
            if expire.checked_duration_since(now).is_none() {
                return Err(io::Error::new(
                    io::ErrorKind::Other,
                    "connection has expired",
                ));
            }
        }
        send_req_header_to_origin(&mut self.inner, req).await
    }
}
