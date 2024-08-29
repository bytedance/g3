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

use anyhow::anyhow;
use slog::slog_info;
use tokio::io::AsyncWriteExt;

use g3_dpi::ProtocolInspectPolicy;
use g3_io_ext::{LineRecvBuf, OnceBufReader};
use g3_slog_types::{LtHost, LtUpstreamAddr, LtUuid};
use g3_smtp_proto::command::Command;
use g3_smtp_proto::response::{ReplyCode, ResponseEncoder, ResponseParser};
use g3_types::net::{Host, UpstreamAddr};

use super::StartTlsProtocol;
#[cfg(feature = "quic")]
use crate::audit::{DetourAction, StreamDetourContext};
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
use initiation::{InitializedExtensions, Initiation};

mod forward;
use forward::{Forward, ForwardNextAction};

mod transaction;
use transaction::Transaction;

#[derive(Default)]
struct SmtpRelayBuf {
    cmd_recv_buf: LineRecvBuf<{ Command::MAX_LINE_SIZE }>,
    rsp_recv_buf: LineRecvBuf<{ ResponseParser::MAX_LINE_SIZE }>,
}

macro_rules! intercept_log {
    ($obj:tt, $($args:tt)+) => {
        slog_info!($obj.ctx.intercept_logger(), $($args)+;
            "intercept_type" => "SmtpConnection",
            "task_id" => LtUuid($obj.ctx.server_task_id()),
            "depth" => $obj.ctx.inspection_depth,
            "upstream" => LtUpstreamAddr(&$obj.upstream),
            "client_host" => $obj.client_host.as_ref().map(LtHost),
            "transaction_count" => $obj.transaction_count,
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
    transaction_count: usize,
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
            transaction_count: 0,
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
        let r = match self.ctx.smtp_inspect_policy() {
            ProtocolInspectPolicy::Intercept => self.do_intercept().await,
            #[cfg(feature = "quic")]
            ProtocolInspectPolicy::Detour => self.do_detour().await.map(|_| None),
            ProtocolInspectPolicy::Bypass => self.do_bypass().await.map(|_| None),
            ProtocolInspectPolicy::Block => self.do_block().await.map(|_| None),
        };
        match r {
            Ok(obj) => {
                intercept_log!(self, "finished");
                Ok(obj)
            }
            Err(e) => {
                intercept_log!(self, "{e}");
                Err(e)
            }
        }
    }

    #[cfg(feature = "quic")]
    async fn do_detour(&mut self) -> ServerTaskResult<()> {
        let Some(client) = self.ctx.audit_handle.stream_detour_client() else {
            return self.do_bypass().await;
        };

        let mut detour_stream = match client.open_detour_stream().await {
            Ok(s) => s,
            Err(e) => {
                self.close_on_detour_error().await;
                return Err(ServerTaskError::InternalAdapterError(e));
            }
        };

        let detour_ctx = StreamDetourContext::new(
            &self.ctx.server_config,
            &self.ctx.server_quit_policy,
            &self.ctx.task_notes,
            &self.upstream,
            g3_dpi::Protocol::Smtp,
        );

        match detour_ctx.check_detour_action(&mut detour_stream).await {
            Ok(DetourAction::Continue) => {
                let SmtpIo {
                    clt_r,
                    clt_w,
                    ups_r,
                    ups_w,
                } = self.io.take().unwrap();

                detour_ctx
                    .relay(clt_r, clt_w, ups_r, ups_w, detour_stream)
                    .await
            }
            Ok(DetourAction::Bypass) => {
                detour_stream.finish();
                self.do_bypass().await
            }
            Ok(DetourAction::Block) => {
                detour_stream.finish();
                self.do_block().await
            }
            Err(e) => {
                detour_stream.finish();
                self.close_on_detour_error().await;
                Err(ServerTaskError::InternalAdapterError(e))
            }
        }
    }

    async fn close_on_detour_error(&mut self) {
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

        if ResponseEncoder::internal_server_error(local_ip)
            .write(&mut clt_w)
            .await
            .is_ok()
        {
            let _ = EndWaitClient::new(local_ip)
                .run_to_end(
                    clt_r,
                    clt_w,
                    self.ctx.smtp_interception().command_wait_timeout,
                )
                .await;
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
        EndWaitClient::new(local_ip)
            .run_to_end(
                clt_r,
                clt_w,
                self.ctx.smtp_interception().command_wait_timeout,
            )
            .await?;
        Err(ServerTaskError::InternalAdapterError(anyhow!(
            "smtp blocked by inspection policy"
        )))
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
            let quit_wait_timeout = interception_config.quit_wait_timeout;
            tokio::spawn(async move {
                let _ = EndQuitServer::run_to_end(ups_r, ups_w, quit_wait_timeout).await;
            });
            return EndWaitClient::new(local_ip)
                .run_to_end(clt_r, clt_w, interception_config.command_wait_timeout)
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
        let (client_host, mut server_ext) = initiation.into_parts();
        self.client_host = Some(client_host);

        let mut relay_buf = SmtpRelayBuf::default();

        loop {
            let allow_odmr = server_ext.allow_odmr(interception_config);
            let allow_starttls = server_ext.allow_starttls(self.from_starttls);
            let mut forward =
                Forward::new(interception_config, local_ip, allow_odmr, allow_starttls);
            let next_action = forward
                .relay(
                    &mut relay_buf,
                    &mut clt_r,
                    &mut clt_w,
                    &mut ups_r,
                    &mut ups_w,
                )
                .await?;
            match next_action {
                ForwardNextAction::Quit => return Ok(None),
                ForwardNextAction::StartTls => {
                    return if let Some(tls_interception) = self.ctx.tls_interception() {
                        let mut start_tls_obj =
                            crate::inspect::start_tls::StartTlsInterceptObject::new(
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
                ForwardNextAction::SetExtensions(ext) => server_ext = ext,
                ForwardNextAction::MailTransport(param) => {
                    let allow_chunking = server_ext.allow_chunking(interception_config);
                    let allow_burl = server_ext.allow_burl(interception_config);

                    let transaction_id = self.transaction_count;
                    self.transaction_count += 1;
                    let mut transaction = Transaction::new(
                        &self.ctx,
                        transaction_id,
                        local_ip,
                        allow_chunking,
                        allow_burl,
                        param,
                    );
                    transaction
                        .relay(
                            &mut relay_buf,
                            &mut clt_r,
                            &mut clt_w,
                            &mut ups_r,
                            &mut ups_w,
                        )
                        .await?;
                    if transaction.quit() {
                        return Ok(None);
                    }
                }
            }
        }
    }
}
