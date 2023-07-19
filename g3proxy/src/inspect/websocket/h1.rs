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

use slog::slog_info;

use g3_slog_types::{LtUpstreamAddr, LtUuid};
use g3_types::net::UpstreamAddr;

use crate::config::server::ServerConfig;
use crate::inspect::{BoxAsyncRead, BoxAsyncWrite, StreamInspectContext};
use crate::serve::ServerTaskResult;

macro_rules! intercept_log {
    ($obj:tt, $($args:tt)+) => {
        slog_info!($obj.ctx.intercept_logger(), $($args)+;
            "intercept_type" => "H1Websocket",
            "task_id" => LtUuid($obj.ctx.server_task_id()),
            "depth" => $obj.ctx.inspection_depth,
            "upstream" => LtUpstreamAddr(&$obj.upstream),
        )
    };
}

struct H1WebsocketIo {
    pub(crate) clt_r: BoxAsyncRead,
    pub(crate) clt_w: BoxAsyncWrite,
    pub(crate) ups_r: BoxAsyncRead,
    pub(crate) ups_w: BoxAsyncWrite,
}

pub(crate) struct H1WebsocketInterceptObject<SC: ServerConfig> {
    io: Option<H1WebsocketIo>,
    pub(crate) ctx: StreamInspectContext<SC>,
    upstream: UpstreamAddr,
}

impl<SC: ServerConfig> H1WebsocketInterceptObject<SC> {
    pub(crate) fn new(ctx: StreamInspectContext<SC>, upstream: UpstreamAddr) -> Self {
        H1WebsocketInterceptObject {
            io: None,
            ctx,
            upstream,
        }
    }

    pub(crate) fn set_io(
        &mut self,
        clt_r: BoxAsyncRead,
        clt_w: BoxAsyncWrite,
        ups_r: BoxAsyncRead,
        ups_w: BoxAsyncWrite,
    ) {
        let io = H1WebsocketIo {
            clt_r,
            clt_w,
            ups_r,
            ups_w,
        };
        self.io = Some(io);
    }

    pub(crate) async fn intercept(mut self) -> ServerTaskResult<()> {
        if let Err(e) = self.do_intercept().await {
            intercept_log!(self, "{e}");
            Err(e)
        } else {
            intercept_log!(self, "finished");
            Ok(())
        }
    }

    async fn do_intercept(&mut self) -> ServerTaskResult<()> {
        let H1WebsocketIo {
            clt_r,
            clt_w,
            ups_r,
            ups_w,
        } = self.io.take().unwrap();

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
