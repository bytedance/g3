/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::time::Duration;

use slog::slog_info;
use tokio::io::{AsyncRead, AsyncWrite, AsyncWriteExt};
use tokio::time::Instant;

use g3_daemon::server::ServerQuitPolicy;
use g3_dpi::{MaybeProtocol, ProtocolInspectionConfig, ProtocolInspector};
use g3_io_ext::{IdleInterval, LimitedCopy, LimitedCopyConfig, LimitedCopyError, OptionalInterval};
use g3_slog_types::LtUuid;
use g3_types::net::UpstreamAddr;

use super::{StreamInspectContext, StreamInspection};
use crate::auth::User;
use crate::config::server::ServerConfig;
use crate::log::task::TaskEvent;
use crate::serve::{ServerTaskError, ServerTaskForbiddenError, ServerTaskResult};

mod object;
pub(crate) use object::StreamInspectObject;

pub(crate) trait StreamTransitTask {
    fn copy_config(&self) -> LimitedCopyConfig;
    fn idle_check_interval(&self) -> IdleInterval;
    fn max_idle_count(&self) -> usize;
    fn log_client_shutdown(&self);
    fn log_upstream_shutdown(&self);
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

    async fn transit_transparent2<CR, CW, UR, UW>(
        &self,
        mut clt_to_ups: LimitedCopy<'_, CR, UW>,
        mut ups_to_clt: LimitedCopy<'_, UR, CW>,
    ) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Unpin,
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        let mut idle_interval = self.idle_check_interval();
        let mut log_interval = self
            .log_flush_interval()
            .map(|log_interval| {
                let interval =
                    tokio::time::interval_at(Instant::now() + log_interval, log_interval);
                OptionalInterval::with(interval)
            })
            .unwrap_or_default();
        let mut idle_count = 0;
        let max_idle_count = self
            .user()
            .and_then(|u| u.task_max_idle_count())
            .unwrap_or(self.max_idle_count());
        loop {
            tokio::select! {
                r = &mut clt_to_ups => {
                    return match r {
                        Ok(_) => {
                            let _ = clt_to_ups.writer().shutdown().await;
                            self.log_client_shutdown();
                            self.transit_south(ups_to_clt, log_interval, idle_interval, idle_count, max_idle_count).await
                        }
                        Err(LimitedCopyError::ReadFailed(e)) => Err(ServerTaskError::ClientTcpReadFailed(e)),
                        Err(LimitedCopyError::WriteFailed(e)) => {
                            let _ = ups_to_clt.write_flush().await;
                            Err(ServerTaskError::UpstreamWriteFailed(e))
                        }
                    };
                }
                r = &mut ups_to_clt => {
                    return match r {
                        Ok(_) => {
                            let _ = ups_to_clt.writer().shutdown().await;
                            self.log_upstream_shutdown();
                            self.transit_north(clt_to_ups, log_interval, idle_interval, idle_count, max_idle_count).await
                        }
                        Err(LimitedCopyError::ReadFailed(e)) => Err(ServerTaskError::UpstreamReadFailed(e)),
                        Err(LimitedCopyError::WriteFailed(e)) => {
                            let _ = clt_to_ups.write_flush().await;
                            Err(ServerTaskError::ClientTcpWriteFailed(e))
                        }
                    };
                }
                _ = log_interval.tick() => {
                    self.log_periodic();
                }
                n = idle_interval.tick() => {
                    if clt_to_ups.is_idle() && ups_to_clt.is_idle() {
                        idle_count += n;

                        if let Some(user) = self.user() {
                            if user.is_blocked() {
                                return Err(ServerTaskError::CanceledAsUserBlocked);
                            }
                        }

                        if idle_count >= max_idle_count {
                            return Err(ServerTaskError::Idle(idle_interval.period(), idle_count));
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
            }
        }
    }

    async fn transit_north<CR, UW>(
        &self,
        mut clt_to_ups: LimitedCopy<'_, CR, UW>,
        mut log_interval: OptionalInterval,
        mut idle_interval: IdleInterval,
        mut idle_count: usize,
        max_idle_count: usize,
    ) -> ServerTaskResult<()>
    where
        CR: AsyncRead + Unpin,
        UW: AsyncWrite + Unpin,
    {
        loop {
            tokio::select! {
                r = &mut clt_to_ups => {
                    return match r {
                        Ok(_) => {
                            let _ = clt_to_ups.writer().shutdown().await;
                            Ok(())
                        }
                        Err(LimitedCopyError::ReadFailed(e)) => Err(ServerTaskError::ClientTcpReadFailed(e)),
                        Err(LimitedCopyError::WriteFailed(e)) => Err(ServerTaskError::UpstreamWriteFailed(e)),
                    };
                }
                _ = log_interval.tick() => {
                    self.log_periodic();
                }
                n = idle_interval.tick() => {
                    if clt_to_ups.is_idle() {
                        idle_count += n;

                        if let Some(user) = self.user() {
                            if user.is_blocked() {
                                return Err(ServerTaskError::CanceledAsUserBlocked);
                            }
                        }

                        if idle_count >= max_idle_count {
                            return Err(ServerTaskError::Idle(idle_interval.period(), idle_count));
                        }
                    } else {
                        idle_count = 0;

                        clt_to_ups.reset_active();
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
            }
        }
    }

    async fn transit_south<CW, UR>(
        &self,
        mut ups_to_clt: LimitedCopy<'_, UR, CW>,
        mut log_interval: OptionalInterval,
        mut idle_interval: IdleInterval,
        mut idle_count: usize,
        max_idle_count: usize,
    ) -> ServerTaskResult<()>
    where
        CW: AsyncWrite + Unpin,
        UR: AsyncRead + Unpin,
    {
        loop {
            tokio::select! {
                r = &mut ups_to_clt => {
                    return match r {
                        Ok(_) => {
                            let _ = ups_to_clt.writer().shutdown().await;
                            Ok(())
                        }
                        Err(LimitedCopyError::ReadFailed(e)) => Err(ServerTaskError::UpstreamReadFailed(e)),
                        Err(LimitedCopyError::WriteFailed(e)) => Err(ServerTaskError::ClientTcpWriteFailed(e)),
                    };
                }
                _ = log_interval.tick() => {
                    self.log_periodic();
                }
                n = idle_interval.tick() => {
                    if ups_to_clt.is_idle() {
                        idle_count += n;

                        if let Some(user) = self.user() {
                            if user.is_blocked() {
                                return Err(ServerTaskError::CanceledAsUserBlocked);
                            }
                        }

                        if idle_count >= max_idle_count {
                            return Err(ServerTaskError::Idle(idle_interval.period(), idle_count));
                        }
                    } else {
                        idle_count = 0;

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
            }
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

enum InspectStatus {
    Bypass,
    Unknown,
    Timeout,
}

impl InspectStatus {
    fn as_str(&self) -> &'static str {
        match self {
            InspectStatus::Bypass => "Bypass",
            InspectStatus::Unknown => "Unknown",
            InspectStatus::Timeout => "Timeout",
        }
    }
}

struct UnknownStreamTransitTask<'a, SC: ServerConfig> {
    ctx: &'a StreamInspectContext<SC>,
    inspect_status: InspectStatus,
}

impl<SC: ServerConfig> UnknownStreamTransitTask<'_, SC> {
    fn log_partial_shutdown(&self, task_event: TaskEvent) {
        if let Some(logger) = self.ctx.intercept_logger() {
            slog_info!(logger, "";
                "intercept_type" => "TransitUnknown",
                "inspect_status" => self.inspect_status.as_str(),
                "task_id" => LtUuid(self.ctx.server_task_id()),
                "task_event" => task_event.as_str(),
                "depth" => self.ctx.inspection_depth,
            );
        }
    }
}

impl<SC: ServerConfig> StreamTransitTask for UnknownStreamTransitTask<'_, SC> {
    fn copy_config(&self) -> LimitedCopyConfig {
        self.ctx.server_config.limited_copy_config()
    }

    fn idle_check_interval(&self) -> IdleInterval {
        self.ctx.idle_wheel.register()
    }

    fn max_idle_count(&self) -> usize {
        self.ctx.max_idle_count
    }

    fn log_client_shutdown(&self) {
        self.log_partial_shutdown(TaskEvent::ClientShutdown);
    }

    fn log_upstream_shutdown(&self) {
        self.log_partial_shutdown(TaskEvent::UpstreamShutdown);
    }

    fn log_periodic(&self) {
        // TODO
    }

    fn log_flush_interval(&self) -> Option<Duration> {
        self.ctx.server_config.task_log_flush_interval()
    }

    fn quit_policy(&self) -> &ServerQuitPolicy {
        self.ctx.server_quit_policy.as_ref()
    }

    fn user(&self) -> Option<&User> {
        self.ctx.user()
    }
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

    pub(super) async fn transit_inspect_unknown<CR, CW, UR, UW>(
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

        let task = UnknownStreamTransitTask {
            ctx: self,
            inspect_status: InspectStatus::Unknown,
        };
        task.transit_transparent(clt_r, clt_w, ups_r, ups_w).await
    }

    pub(super) async fn transit_inspect_timeout<CR, CW, UR, UW>(
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

        let task = UnknownStreamTransitTask {
            ctx: self,
            inspect_status: InspectStatus::Timeout,
        };
        task.transit_transparent(clt_r, clt_w, ups_r, ups_w).await
    }

    pub(super) async fn transit_inspect_bypass<CR, CW, UR, UW>(
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
        let task = UnknownStreamTransitTask {
            ctx: self,
            inspect_status: InspectStatus::Bypass,
        };
        task.transit_transparent(clt_r, clt_w, ups_r, ups_w).await
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
                    return stream.transit_inspect_unknown().await;
                }
                StreamInspection::StreamInspect(stream) => {
                    if stream.ctx.skip_next_inspection() {
                        return stream.transit_inspect_unknown().await;
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
