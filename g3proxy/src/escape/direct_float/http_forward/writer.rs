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

use super::DirectFloatBindIp;
use crate::auth::UserUpstreamTrafficStats;
use crate::escape::direct_fixed::DirectFixedEscaperStats;
use crate::module::http_forward::{
    ArcHttpForwardTaskRemoteStats, HttpForwardRemoteWrapperStats,
    HttpForwardTaskRemoteWrapperStats, HttpForwardWrite, send_req_header_to_origin,
};
use crate::serve::ServerTaskNotes;

pin_project! {
    pub(super) struct DirectFloatHttpForwardWriter<W: AsyncWrite> {
        bind: DirectFloatBindIp,
        #[pin]
        inner: W,
        escaper_stats: Option<Arc<DirectFixedEscaperStats>>,
    }
}

impl<W> DirectFloatHttpForwardWriter<W>
where
    W: AsyncWrite,
{
    pub(super) fn new(
        ups_w: W,
        escaper_stats: Option<Arc<DirectFixedEscaperStats>>,
        bind: DirectFloatBindIp,
    ) -> Self {
        DirectFloatHttpForwardWriter {
            bind,
            inner: ups_w,
            escaper_stats,
        }
    }
}

impl<W> AsyncWrite for DirectFloatHttpForwardWriter<W>
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
impl<W> HttpForwardWrite for DirectFloatHttpForwardWriter<LimitedWriter<W>>
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
            let mut wrapper_stats =
                HttpForwardRemoteWrapperStats::new(escaper_stats.clone(), task_stats);
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
        if self.bind.is_expired() {
            Err(io::Error::other("connection has expired"))
        } else {
            send_req_header_to_origin(&mut self.inner, req, body).await
        }
    }
}
