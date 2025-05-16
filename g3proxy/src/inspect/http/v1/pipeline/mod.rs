/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::sync::Arc;

use tokio::io::AsyncRead;
use tokio::sync::mpsc;

use super::{H1InterceptionError, HttpRequestIo};
use crate::config::server::ServerConfig;
use crate::inspect::StreamInspectContext;

mod stats;
pub(super) use stats::PipelineStats;

mod request;
pub(super) use request::{HttpRecvRequest, HttpRequest};
use request::{HttpRequestAcceptor, HttpRequestForwarder};

pub(super) fn new_request_handler<SC, R>(
    ctx: StreamInspectContext<SC>,
    req_io: HttpRequestIo<R>,
    stats: Arc<PipelineStats>,
) -> (HttpRequestForwarder<SC, R>, HttpRequestAcceptor<R>)
where
    SC: ServerConfig,
    R: AsyncRead + Unpin,
{
    let (send_request, recv_request) = mpsc::channel(ctx.h1_interception().pipeline_size.get());

    let forwarder = HttpRequestForwarder::new(ctx, req_io, send_request, stats);
    let acceptor = HttpRequestAcceptor::new(recv_request);

    (forwarder, acceptor)
}
