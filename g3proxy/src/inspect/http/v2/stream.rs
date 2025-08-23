/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::str::FromStr;

use bytes::Bytes;
use h2::RecvStream;
use h2::client::SendRequest;
use h2::ext::Protocol;
use h2::server::SendResponse;
use http::{Method, Request};

use g3_types::net::HttpUpgradeToken;

use super::{H2ConnectTask, H2ExtendedConnectTask, H2ForwardTask};
use crate::config::server::ServerConfig;
use crate::inspect::StreamInspectContext;

pub(super) async fn transfer<SC>(
    mut clt_req: Request<RecvStream>,
    clt_send_rsp: SendResponse<Bytes>,
    h2s: SendRequest<Bytes>,
    ctx: StreamInspectContext<SC>,
) where
    SC: ServerConfig + Send + Sync + 'static,
{
    if ctx.h1_interception().steal_forwarded_for {
        clt_req.headers_mut().remove(http::header::FORWARDED);
        clt_req.headers_mut().remove("x-forwarded-for");
    }
    let clt_stream_id = clt_send_rsp.stream_id();
    if clt_req.method().eq(&Method::CONNECT) {
        if let Some(protocol) = clt_req.extensions().get::<Protocol>() {
            let upgrade_protocol = HttpUpgradeToken::from_str(protocol.as_str())
                .unwrap_or_else(|_e| HttpUpgradeToken::Unsupported(protocol.as_str().to_string()));

            let connect_task = H2ExtendedConnectTask::new(ctx, clt_stream_id, upgrade_protocol);
            connect_task.into_running(clt_req, clt_send_rsp, h2s).await;
        } else {
            let connect_task = H2ConnectTask::new(ctx, clt_stream_id);
            connect_task.into_running(clt_req, clt_send_rsp, h2s).await
        };
    } else {
        let forward_task = H2ForwardTask::new(ctx, clt_stream_id, &clt_req);
        forward_task.forward(clt_req, clt_send_rsp, h2s).await
    }
}
