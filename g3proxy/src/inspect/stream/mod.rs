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

use std::time::Duration;

use tokio::io::{AsyncRead, AsyncWrite};
use tokio::time::Instant;

use g3_daemon::server::ServerQuitPolicy;
use g3_dpi::{MaybeProtocol, ProtocolInspectionConfig, ProtocolInspector};
use g3_io_ext::{LimitedCopy, LimitedCopyConfig, LimitedCopyError, OptionalInterval};
use g3_types::net::UpstreamAddr;

use super::{StreamInspectContext, StreamInspection};
use crate::auth::User;
use crate::config::server::ServerConfig;
use crate::serve::{ServerTaskError, ServerTaskForbiddenError, ServerTaskResult};

mod object;
pub(crate) use object::StreamInspectObject;

pub(crate) trait StreamTransitTask {
    fn copy_config(&self) -> LimitedCopyConfig;
    fn idle_check_interval(&self) -> Duration;
    fn max_idle_count(&self) -> i32;
    fn log_periodic(&self);
    fn log_flush_interval(&self) -> Option<Duration>;
    fn quit_policy(&self) -> &ServerQuitPolicy;
    fn user(&self) -> Option<&User>;

    async fn transit_transparent<CR, CW, UR, UW>(
        &self,
        mut clt_r: CR,
        mut clt_w: CW,
        mut ups_r: UR,
        mut ups_w: UW,
    ) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        let copy_config = self.copy_config();
        let clt_to_ups = LimitedCopy::new(&mut clt_r, &mut ups_w, &copy_config);
        let ups_to_clt = LimitedCopy::new(&mut ups_r, &mut clt_w, &copy_config);

        self.transit_transparent2(clt_to_ups, ups_to_clt).await
    }

    async fn transit_transparent2<'a, CR, CW, UR, UW>(
        &self,
        mut clt_to_ups: LimitedCopy<'a, CR, UW>,
        mut ups_to_clt: LimitedCopy<'a, UR, CW>,
    ) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        let idle_duration = self.idle_check_interval();
        let mut idle_interval =
            tokio::time::interval_at(Instant::now() + idle_duration, idle_duration);
        let mut log_interval = self
            .log_flush_interval()
            .map(|log_interval| {
                let interval =
                    tokio::time::interval_at(Instant::now() + log_interval, log_interval);
                OptionalInterval::with(interval)
            })
            .unwrap_or_default();
        let mut idle_count = 0;
        loop {
            tokio::select! {
                biased;

                r = &mut clt_to_ups => {
                    let _ = ups_to_clt.write_flush().await;
                    return match r {
                        Ok(_) => Err(ServerTaskError::ClosedByClient),
                        Err(LimitedCopyError::ReadFailed(e)) => Err(ServerTaskError::ClientTcpReadFailed(e)),
                        Err(LimitedCopyError::WriteFailed(e)) => Err(ServerTaskError::UpstreamWriteFailed(e)),
                    };
                }
                r = &mut ups_to_clt => {
                    let _ = clt_to_ups.write_flush().await;
                    return match r {
                        Ok(_) => Err(ServerTaskError::ClosedByUpstream),
                        Err(LimitedCopyError::ReadFailed(e)) => Err(ServerTaskError::UpstreamReadFailed(e)),
                        Err(LimitedCopyError::WriteFailed(e)) => Err(ServerTaskError::ClientTcpWriteFailed(e)),
                    };
                }
                _ = log_interval.tick() => {
                    self.log_periodic();
                }
                _ = idle_interval.tick() => {
                    if clt_to_ups.is_idle() && ups_to_clt.is_idle() {
                        idle_count += 1;

                        let quit = if let Some(user) = self.user() {
                            if user.is_blocked() {
                                return Err(ServerTaskError::CanceledAsUserBlocked);
                            }
                            idle_count >= user.task_max_idle_count()
                        } else {
                            idle_count >= self.max_idle_count()
                        };

                        if quit {
                            return Err(ServerTaskError::Idle(idle_duration, idle_count));
                        }
                    } else {
                        idle_count = 0;

                        clt_to_ups.reset_active();
                        ups_to_clt.reset_active();
                    }

                    if let Some(user) = self.user() {
                        if user.is_blocked() {
                            return Err(ServerTaskError::CanceledAsUserBlocked);
                        }
                    }

                    if self.quit_policy().force_quit() {
                        return Err(ServerTaskError::CanceledAsServerQuit)
                    }
                }
            };
        }
    }
}

pub(crate) async fn transit_with_inspection<CR, CW, UR, UW, SC>(
    clt_r: CR,
    clt_w: CW,
    ups_r: UR,
    ups_w: UW,
    ctx: StreamInspectContext<SC>,
    upstream: UpstreamAddr,
    explicit_protocol: Option<MaybeProtocol>,
) -> ServerTaskResult<()>
where
    CR: AsyncRead + Send + Sync + Unpin + 'static,
    CW: AsyncWrite + Send + Sync + Unpin + 'static,
    UR: AsyncRead + Send + Sync + Unpin + 'static,
    UW: AsyncWrite + Send + Sync + Unpin + 'static,
    SC: ServerConfig + Send + Sync + 'static,
{
    let inspector = ctx.protocol_inspector(explicit_protocol);

    let mut obj = StreamInspectObject::new(ctx, upstream);
    obj.set_io(
        Box::new(clt_r),
        Box::new(clt_w),
        Box::new(ups_r),
        Box::new(ups_w),
    );
    StreamInspection::StreamInspect(obj)
        .into_loop_inspection(inspector)
        .await
}

impl<SC> StreamInspectContext<SC>
where
    SC: ServerConfig,
{
    #[inline]
    fn protocol_inspection(&self) -> &ProtocolInspectionConfig {
        self.audit_handle.protocol_inspection()
    }

    #[inline]
    fn skip_next_inspection(&self) -> bool {
        self.inspection_depth >= self.protocol_inspection().max_depth()
    }

    pub(super) async fn transit_unknown<CR, CW, UR, UW>(
        &self,
        clt_r: CR,
        clt_w: CW,
        ups_r: UR,
        ups_w: UW,
    ) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        if let Some(user_ctx) = &self.task_notes.user_ctx {
            if user_ctx.user.audit().prohibit_unknown_protocol {
                user_ctx.forbidden_stats.add_proto_banned();
                return Err(ServerTaskError::ForbiddenByRule(
                    ServerTaskForbiddenError::ProtoBanned,
                ));
            }
        }

        self.transit_transparent(clt_r, clt_w, ups_r, ups_w).await
    }

    pub(super) async fn transit_unknown_timeout<CR, CW, UR, UW>(
        &self,
        clt_r: CR,
        clt_w: CW,
        ups_r: UR,
        ups_w: UW,
    ) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        if let Some(user_ctx) = &self.task_notes.user_ctx {
            if user_ctx.user.audit().prohibit_timeout_protocol {
                user_ctx.forbidden_stats.add_proto_banned();
                return Err(ServerTaskError::ForbiddenByRule(
                    ServerTaskForbiddenError::ProtoBanned,
                ));
            }
        }

        self.transit_transparent(clt_r, clt_w, ups_r, ups_w).await
    }

    pub(crate) async fn transit_transparent<CR, CW, UR, UW>(
        &self,
        mut clt_r: CR,
        mut clt_w: CW,
        mut ups_r: UR,
        mut ups_w: UW,
    ) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        let copy_config = self.server_config.limited_copy_config();
        let mut clt_to_ups = LimitedCopy::new(&mut clt_r, &mut ups_w, &copy_config);
        let mut ups_to_clt = LimitedCopy::new(&mut ups_r, &mut clt_w, &copy_config);

        let idle_duration = self.server_config.task_idle_check_duration();
        let mut idle_interval =
            tokio::time::interval_at(Instant::now() + idle_duration, idle_duration);
        let mut idle_count = 0;
        loop {
            tokio::select! {
                biased;

                r = &mut clt_to_ups => {
                    let _ = ups_to_clt.write_flush().await;
                    return match r {
                        Ok(_) => Err(ServerTaskError::ClosedByClient),
                        Err(LimitedCopyError::ReadFailed(e)) => Err(ServerTaskError::ClientTcpReadFailed(e)),
                        Err(LimitedCopyError::WriteFailed(e)) => Err(ServerTaskError::UpstreamWriteFailed(e)),
                    };
                }
                r = &mut ups_to_clt => {
                    let _ = clt_to_ups.write_flush().await;
                    return match r {
                        Ok(_) => Err(ServerTaskError::ClosedByUpstream),
                        Err(LimitedCopyError::ReadFailed(e)) => Err(ServerTaskError::UpstreamReadFailed(e)),
                        Err(LimitedCopyError::WriteFailed(e)) => Err(ServerTaskError::ClientTcpWriteFailed(e)),
                    };
                }
                _ = idle_interval.tick() => {
                    if clt_to_ups.is_idle() && ups_to_clt.is_idle() {
                        idle_count += 1;

                        let quit = if let Some(user) = self.user() {
                            if user.is_blocked() {
                                return Err(ServerTaskError::CanceledAsUserBlocked);
                            }
                            idle_count >= user.task_max_idle_count()
                        } else {
                            idle_count >= self.server_config.task_max_idle_count()
                        };

                        if quit {
                            return Err(ServerTaskError::Idle(idle_duration, idle_count));
                        }
                    } else {
                        idle_count = 0;

                        clt_to_ups.reset_active();
                        ups_to_clt.reset_active();
                    }

                    if let Some(user) = self.user() {
                        if user.is_blocked() {
                            return Err(ServerTaskError::CanceledAsUserBlocked);
                        }
                    }

                    if self.server_quit_policy.force_quit() {
                        return Err(ServerTaskError::CanceledAsServerQuit)
                    }
                }
            };
        }
    }
}

impl<SC> StreamInspection<SC>
where
    SC: ServerConfig + Send + Sync + 'static,
{
    pub(crate) async fn into_loop_inspection(
        self,
        mut inspector: ProtocolInspector,
    ) -> ServerTaskResult<()> {
        let mut obj = self;

        loop {
            match obj {
                StreamInspection::StreamUnknown(stream) => {
                    return stream.transit_unknown().await;
                }
                StreamInspection::StreamInspect(stream) => {
                    if stream.ctx.skip_next_inspection() {
                        return stream.transit_unknown().await;
                    }

                    obj = stream.transit_with_inspection(&mut inspector).await?;
                    inspector.unset_no_explicit_ssl();
                }
                StreamInspection::TlsModern(tls) => {
                    obj = tls.intercept_modern(&mut inspector).await?;
                    inspector.reset_state();
                    inspector.set_no_explicit_ssl();
                }
                #[cfg(feature = "vendored-tongsuo")]
                StreamInspection::TlsTlcp(tlcp) => {
                    obj = tlcp.intercept_tlcp(&mut inspector).await?;
                    inspector.reset_state();
                    inspector.set_no_explicit_ssl();
                }
                StreamInspection::StartTls(start_tls) => {
                    obj = start_tls.intercept().await?;
                    // no need to reset inspector state as the protocol should be known
                }
                StreamInspection::H1(h1) => match h1.intercept().await? {
                    Some(new_obj) => {
                        obj = new_obj;
                        inspector.reset_state();
                        inspector.unset_no_explicit_ssl();
                    }
                    None => break,
                },
                StreamInspection::H2(h2) => {
                    return h2.intercept().await;
                }
                StreamInspection::Websocket(websocket) => {
                    return websocket.intercept().await;
                }
                StreamInspection::Smtp(smtp) => match smtp.intercept().await? {
                    Some(new_obj) => {
                        obj = new_obj;
                        // no need to reset inspector state as the protocol should be known
                    }
                    None => break,
                },
                StreamInspection::Imap(imap) => match imap.intercept().await? {
                    Some(new_obj) => {
                        obj = new_obj;
                        // no need to reset inspector state as the protocol should be known
                    }
                    None => break,
                },
                StreamInspection::End => break,
            }
        }

        Ok(())
    }
}
