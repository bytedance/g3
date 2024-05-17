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
use tokio::io::AsyncWriteExt;

use g3_dpi::ProtocolInspectPolicy;
use g3_io_ext::OnceBufReader;
use g3_slog_types::{LtHost, LtUpstreamAddr, LtUuid};
use g3_smtp_proto::response::{ReplyCode, ResponseEncoder};
use g3_types::net::{Host, UpstreamAddr};

use super::StartTlsProtocol;
use crate::config::server::ServerConfig;
use crate::inspect::{BoxAsyncRead, BoxAsyncWrite, StreamInspectContext, StreamInspection};
use crate::serve::{ServerTaskError, ServerTaskResult};

mod ext;
use ext::{CommandLineRecvExt, ResponseLineRecvExt, ResponseParseExt};

mod greeting;
use greeting::Greeting;

mod ending;
use ending::{EndQuitServer, EndWaitClient};

mod initiation;
use initiation::Initiation;

mod forward;
use forward::{Forward, ForwardNextAction};

macro_rules! intercept_log {
    ($obj:tt, $($args:tt)+) => {
        slog_info!($obj.ctx.intercept_logger(), $($args)+;
            "intercept_type" => "SMTP",
            "task_id" => LtUuid($obj.ctx.server_task_id()),
            "depth" => $obj.ctx.inspection_depth,
            "upstream" => LtUpstreamAddr(&$obj.upstream),
            "client_host" => $obj.client_host.as_ref().map(LtHost),
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
    ctx: StreamInspectContext<SC>,
    upstream: UpstreamAddr,
    from_starttls: bool,
    client_host: Option<Host>,
}

impl<SC> SmtpInterceptObject<SC>
where
    SC: ServerConfig + Send + Sync + 'static,
{
    pub(crate) fn new(ctx: StreamInspectContext<SC>, upstream: UpstreamAddr) -> Self {
        SmtpInterceptObject {
            io: None,
            ctx,
            upstream,
            from_starttls: false,
            client_host: None,
        }
    }

    pub(crate) fn set_from_starttls(&mut self) {
        self.from_starttls = true;
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

    pub(crate) async fn intercept(mut self) -> ServerTaskResult<Option<StreamInspection<SC>>> {
        match self.ctx.smtp_inspect_policy() {
            ProtocolInspectPolicy::Bypass => {
                self.do_bypass().await?;
                Ok(None)
            }
            ProtocolInspectPolicy::Intercept => match self.do_intercept().await {
                Ok(obj) => {
                    intercept_log!(self, "finished");
                    Ok(obj)
                }
                Err(e) => {
                    intercept_log!(self, "{e}");
                    Err(e)
                }
            },
            ProtocolInspectPolicy::Block => {
                self.do_block().await?;
                Ok(None)
            }
        }
    }

    async fn do_bypass(&mut self) -> ServerTaskResult<()> {
        let SmtpIo {
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

    async fn do_block(&mut self) -> ServerTaskResult<()> {
        let SmtpIo {
            clt_r,
            mut clt_w,
            ups_r: _,
            mut ups_w,
        } = self.io.take().unwrap();

        tokio::spawn(async move {
            let _ = ups_w.shutdown().await;
        });

        let local_ip = self.ctx.task_notes.server_addr.ip();

        ResponseEncoder::local_service_blocked(local_ip)
            .write(&mut clt_w)
            .await
            .map_err(ServerTaskError::ClientTcpWriteFailed)?;
        EndWaitClient::new(local_ip).run_to_end(clt_r, clt_w).await
    }

    async fn do_intercept(&mut self) -> ServerTaskResult<Option<StreamInspection<SC>>> {
        let SmtpIo {
            clt_r,
            mut clt_w,
            ups_r,
            ups_w,
        } = self.io.take().unwrap();

        if self.from_starttls {
            return self
                .start_initiation(clt_r, clt_w, ups_r.into_inner(), ups_w)
                .await;
        }

        let interception_config = self.ctx.smtp_interception();
        let local_ip = self.ctx.task_notes.server_addr.ip();

        let mut greeting = Greeting::new(local_ip);
        let ups_r = match greeting
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
        self.upstream.set_host(host);
        if code == ReplyCode::NO_SERVICE {
            let timeout = interception_config.quit_wait_timeout;
            tokio::spawn(async move {
                let _ = EndQuitServer::run_to_end(ups_r, ups_w, timeout).await;
            });
            return EndWaitClient::new(local_ip)
                .run_to_end(clt_r, clt_w)
                .await
                .map(|_| None);
        }

        self.start_initiation(clt_r, clt_w, ups_r, ups_w).await
    }

    async fn start_initiation(
        &mut self,
        mut clt_r: BoxAsyncRead,
        mut clt_w: BoxAsyncWrite,
        mut ups_r: BoxAsyncRead,
        mut ups_w: BoxAsyncWrite,
    ) -> ServerTaskResult<Option<StreamInspection<SC>>> {
        let local_ip = self.ctx.task_notes.server_addr.ip();
        let interception_config = self.ctx.smtp_interception();

        let mut initiation = Initiation::new(interception_config, local_ip, self.from_starttls);
        initiation
            .relay(&mut clt_r, &mut clt_w, &mut ups_r, &mut ups_w)
            .await?;
        let (client_host, server_ext) = initiation.into_parts();
        self.client_host = Some(client_host);

        let allow_odmr = server_ext.allow_odmr(interception_config);
        let allow_starttls = server_ext.allow_starttls(self.from_starttls);

        let mut forward = Forward::new(local_ip, allow_odmr, allow_starttls);
        let next_action = forward
            .relay(&mut clt_r, &mut clt_w, &mut ups_r, &mut ups_w)
            .await?;
        match next_action {
            ForwardNextAction::StartTls => {
                return if let Some(tls_interception) = self.ctx.tls_interception() {
                    let mut start_tls_obj = crate::inspect::start_tls::StartTlsInterceptObject::new(
                        self.ctx.clone(),
                        self.upstream.clone(),
                        tls_interception,
                        StartTlsProtocol::Smtp,
                    );
                    start_tls_obj.set_io(clt_r, clt_w, ups_r, ups_w);
                    Ok(Some(StreamInspection::StartTls(start_tls_obj)))
                } else {
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
                    .map(|_| None)
                }
            }
            ForwardNextAction::ReverseConnection => {
                return crate::inspect::stream::transit_transparent(
                    clt_r,
                    clt_w,
                    ups_r,
                    ups_w,
                    &self.ctx.server_config,
                    &self.ctx.server_quit_policy,
                    self.ctx.user(),
                )
                .await
                .map(|_| None);
            }
            ForwardNextAction::MailTransport(_param) => {
                // TODO
            }
        }

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
        .map(|_| None)
    }
}
