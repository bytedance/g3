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

use crate::auth::UserUpstreamTrafficStats;
use crate::module::http_forward::{
    ArcHttpForwardTaskRemoteStats, HttpForwardRemoteWrapperStats, HttpForwardTaskRemoteStats,
    HttpForwardTaskRemoteWrapperStats, HttpForwardWrite, send_req_header_to_origin,
};
use crate::serve::ServerTaskNotes;

pin_project! {
    pub(crate) struct DirectHttpForwardWriter<W: AsyncWrite, S: HttpForwardTaskRemoteStats> {
        #[pin]
        inner: W,
        escaper_stats: Option<Arc<S>>,
    }
}

impl<W, S> DirectHttpForwardWriter<W, S>
where
    W: AsyncWrite,
    S: HttpForwardTaskRemoteStats,
{
    pub(crate) fn new(ups_w: W, escaper_stats: Option<Arc<S>>) -> Self {
        DirectHttpForwardWriter {
            inner: ups_w,
            escaper_stats,
        }
    }
}

impl<W, S> AsyncWrite for DirectHttpForwardWriter<W, S>
where
    W: AsyncWrite,
    S: HttpForwardTaskRemoteStats,
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
impl<W, S> HttpForwardWrite for DirectHttpForwardWriter<LimitedWriter<W>, S>
where
    W: AsyncWrite + Send + Unpin,
    S: HttpForwardTaskRemoteStats + Send + Sync + 'static,
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
        send_req_header_to_origin(&mut self.inner, req, body).await
    }
}
