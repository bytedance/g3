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

use bytes::Bytes;
use h2::{RecvStream, SendStream};
use slog::slog_info;

use g3_daemon::log::types::{LtUpstreamAddr, LtUuid};
use g3_h2::{H2StreamReader, H2StreamWriter};
use g3_types::net::UpstreamAddr;

use crate::config::server::ServerConfig;
use crate::inspect::StreamInspectContext;
use crate::serve::ServerTaskResult;

macro_rules! intercept_log {
    ($obj:tt, $($args:tt)+) => {
        slog_info!($obj.ctx.intercept_logger(), $($args)+;
            "intercept_type" => "H2Websocket",
            "task_id" => LtUuid($obj.ctx.server_task_id()),
            "depth" => $obj.ctx.inspection_depth,
            "upstream" => LtUpstreamAddr(&$obj.upstream),
        )
    };
}

pub(crate) struct H2WebsocketInterceptObject<SC: ServerConfig> {
    ctx: StreamInspectContext<SC>,
    upstream: UpstreamAddr,
}

impl<SC: ServerConfig> H2WebsocketInterceptObject<SC> {
    pub(crate) fn new(ctx: StreamInspectContext<SC>, upstream: UpstreamAddr) -> Self {
        H2WebsocketInterceptObject { ctx, upstream }
    }
}

impl<SC: ServerConfig> H2WebsocketInterceptObject<SC> {
    pub(crate) async fn intercept(
        mut self,
        clt_r: RecvStream,
        clt_w: SendStream<Bytes>,
        ups_r: RecvStream,
        ups_w: SendStream<Bytes>,
    ) {
        if let Err(e) = self.do_intercept(clt_r, clt_w, ups_r, ups_w).await {
            intercept_log!(self, "{e}");
        } else {
            intercept_log!(self, "finished");
        }
    }

    async fn do_intercept(
        &mut self,
        clt_r: RecvStream,
        clt_w: SendStream<Bytes>,
        ups_r: RecvStream,
        ups_w: SendStream<Bytes>,
    ) -> ServerTaskResult<()> {
        let clt_r = H2StreamReader::new(clt_r);
        let clt_w = H2StreamWriter::new(clt_w);
        let ups_r = H2StreamReader::new(ups_r);
        let ups_w = H2StreamWriter::new(ups_w);

        crate::inspect::stream::transit_transparent(
            clt_r,
            clt_w,
            ups_r,
            ups_w,
            &self.ctx.server_config,
            &self.ctx.server_quit_policy,
            self.ctx.user(),
        )
        .await
    }
}
