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

use std::str::FromStr;
use std::sync::Arc;

use bytes::Bytes;
use h2::client::SendRequest;
use h2::server::SendResponse;
use h2::RecvStream;
use http::{Method, Request};

use g3_dpi::Protocol;
use g3_types::net::HttpUpgradeToken;

use super::{H2ConcurrencyStats, H2ConnectTask, H2ExtendedConnectTask, H2ForwardTask};
use crate::config::server::ServerConfig;
use crate::inspect::StreamInspectContext;

pub(super) async fn transfer<SC>(
    clt_req: Request<RecvStream>,
    clt_send_rsp: SendResponse<Bytes>,
    h2s: SendRequest<Bytes>,
    ctx: StreamInspectContext<SC>,
    cstats: Arc<H2ConcurrencyStats>,
) where
    SC: ServerConfig + Send + Sync + 'static,
{
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
        let forward_task = H2ForwardTask::new(ctx, clt_stream_id, cstats, &clt_req);
        forward_task.forward(clt_req, clt_send_rsp, h2s).await
    }
}
