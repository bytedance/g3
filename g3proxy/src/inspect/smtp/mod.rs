/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use g3_io_ext::OnceBufReader;
use g3_slog_types::{LtHost, LtUuid};
use g3_smtp_proto::response::ReplyCode;
use g3_types::net::Host;

use crate::config::server::ServerConfig;
use crate::inspect::{BoxAsyncRead, BoxAsyncWrite, StreamInspectContext};
use crate::serve::ServerTaskResult;

mod ext;
use ext::{CommandLineRecvExt, ResponseLineRecvExt, ResponseParseExt};

mod greeting;
use greeting::Greeting;

mod ending;
use ending::{EndQuitServer, EndWaitClient};

mod initiation;
use initiation::Initiation;

macro_rules! intercept_log {
    ($obj:tt, $($args:tt)+) => {
        slog_info!($obj.ctx.intercept_logger(), $($args)+;
            "intercept_type" => "SMTP",
            "task_id" => LtUuid($obj.ctx.server_task_id()),
            "depth" => $obj.ctx.inspection_depth,
            "upstream_host" => $obj.upstream_host.as_ref().map(LtHost),
        )
    };
}

struct SmtpIo {
    pub(crate) clt_r: BoxAsyncRead,
    pub(crate) clt_w: BoxAsyncWrite,
    pub(crate) ups_r: OnceBufReader<BoxAsyncRead>,
    pub(crate) ups_w: BoxAsyncWrite,
}

pub(crate) struct SmtpInterceptObject<SC: ServerConfig> {
    io: Option<SmtpIo>,
    pub(crate) ctx: StreamInspectContext<SC>,
    upstream_host: Option<Host>,
}

impl<SC: ServerConfig> SmtpInterceptObject<SC> {
    pub(crate) fn new(ctx: StreamInspectContext<SC>) -> Self {
        SmtpInterceptObject {
            io: None,
            ctx,
            upstream_host: None,
        }
    }

    pub(crate) fn set_io(
        &mut self,
        clt_r: BoxAsyncRead,
        clt_w: BoxAsyncWrite,
        ups_r: OnceBufReader<BoxAsyncRead>,
        ups_w: BoxAsyncWrite,
    ) {
        let io = SmtpIo {
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
        let SmtpIo {
            mut clt_r,
            mut clt_w,
            ups_r,
            mut ups_w,
        } = self.io.take().unwrap();

        let interception_config = self.ctx.smtp_interception();
        let local_ip = self.ctx.task_notes.server_addr.ip();

        let mut greeting = Greeting::new(local_ip);
        let mut ups_r = match greeting
            .relay(ups_r, &mut clt_w, interception_config.greeting_timeout)
            .await
        {
            Ok(ups_r) => ups_r,
            Err(e) => {
                greeting.reply_no_service(&e, &mut clt_w).await;
                return Err(e.into());
            }
        };
        let (code, host) = greeting.into_parts();
        self.upstream_host = Some(host);
        if code == ReplyCode::NO_SERVICE {
            let timeout = interception_config.quit_wait_timeout;
            tokio::spawn(async move {
                let _ = EndQuitServer::run_to_end(ups_r, ups_w, timeout).await;
            });
            return EndWaitClient::new(local_ip).run_to_end(clt_r, clt_w).await;
        }

        let initiation = Initiation::new(local_ip);
        initiation
            .relay(&mut clt_r, &mut clt_w, &mut ups_r, &mut ups_w)
            .await?;

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
