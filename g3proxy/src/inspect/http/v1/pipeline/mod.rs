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

use std::sync::Arc;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::mpsc;

use super::{H1InterceptionError, HttpRequestIo};
use crate::config::server::ServerConfig;
use crate::inspect::StreamInspectContext;

mod stats;
pub(super) use stats::PipelineStats;

mod request;
pub(super) use request::{HttpRecvRequest, HttpRequest};
use request::{HttpRequestAcceptor, HttpRequestForwarder};

pub(super) fn new_request_handler<SC, R, W>(
    ctx: StreamInspectContext<SC>,
    req_io: HttpRequestIo<R, W>,
    stats: Arc<PipelineStats>,
) -> (HttpRequestForwarder<SC, R, W>, HttpRequestAcceptor<R, W>)
where
    SC: ServerConfig,
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let (send_request, recv_request) = mpsc::channel(ctx.h1_interception().pipeline_size);

    let forwarder = HttpRequestForwarder::new(ctx, req_io, send_request, stats);
    let acceptor = HttpRequestAcceptor::new(recv_request);

    (forwarder, acceptor)
}
