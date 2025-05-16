/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use slog::Logger;
use tokio::io::{AsyncRead, AsyncWrite};
use uuid::Uuid;

use g3_daemon::server::ServerQuitPolicy;
use g3_dpi::{
    H1InterceptionConfig, H2InterceptionConfig, ImapInterceptionConfig, MaybeProtocol,
    ProtocolInspectAction, ProtocolInspector, SmtpInterceptionConfig,
};
use g3_io_ext::IdleWheel;
use g3_types::net::{Host, OpensslClientConfig};

use crate::audit::AuditHandle;
use crate::auth::{User, UserForbiddenStats, UserSite};
use crate::config::server::ServerConfig;
use crate::module::tcp_connect::TcpConnectTaskNotes;
use crate::serve::{ArcServerStats, ServerIdleChecker, ServerTaskNotes};

mod error;
pub(crate) use error::InterceptionError;

pub(crate) mod stream;
pub(crate) use stream::StreamTransitTask;

pub(crate) mod tls;
use tls::TlsInterceptionContext;

pub(crate) mod start_tls;
use start_tls::StartTlsProtocol;

pub(crate) mod http;
mod websocket;

pub(crate) mod imap;
pub(crate) mod smtp;

#[derive(Clone)]
pub(super) struct StreamInspectUserContext {
    raw_user_name: Option<Arc<str>>,
    user: Arc<User>,
    user_site: Option<Arc<UserSite>>,
    forbidden_stats: Arc<UserForbiddenStats>,
}

impl StreamInspectUserContext {
    fn http_rsp_hdr_recv_timeout(&self) -> Option<Duration> {
        self.user_site
            .as_ref()
            .and_then(|site| site.http_rsp_hdr_recv_timeout())
            .or(self.user.http_rsp_hdr_recv_timeout())
    }
}

#[derive(Clone)]
pub(crate) struct StreamInspectTaskNotes {
    task_id: Uuid,
    pub(crate) client_addr: SocketAddr,
    pub(crate) server_addr: SocketAddr,
    worker_id: Option<usize>,
    user_ctx: Option<StreamInspectUserContext>,
}

impl StreamInspectTaskNotes {
    pub(crate) fn user(&self) -> Option<&Arc<User>> {
        self.user_ctx.as_ref().map(|ctx| &ctx.user)
    }

    pub(crate) fn raw_username(&self) -> Option<&Arc<str>> {
        self.user_ctx
            .as_ref()
            .and_then(|ctx| ctx.raw_user_name.as_ref())
    }

    #[inline]
    pub(crate) fn task_id(&self) -> &Uuid {
        &self.task_id
    }
}

impl From<&ServerTaskNotes> for StreamInspectTaskNotes {
    fn from(task_notes: &ServerTaskNotes) -> Self {
        StreamInspectTaskNotes {
            task_id: task_notes.id,
            client_addr: task_notes.client_addr(),
            server_addr: task_notes.server_addr(),
            worker_id: task_notes.worker_id(),
            user_ctx: task_notes.user_ctx().map(|ctx| StreamInspectUserContext {
                raw_user_name: ctx.raw_user_name().cloned(),
                user: ctx.user().clone(),
                user_site: ctx.user_site().cloned(),
                forbidden_stats: ctx.forbidden_stats().clone(),
            }),
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct StreamInspectConnectNotes {
    pub(crate) client_addr: SocketAddr,
    pub(crate) server_addr: SocketAddr,
}

impl From<&TcpConnectTaskNotes> for StreamInspectConnectNotes {
    fn from(tcp_notes: &TcpConnectTaskNotes) -> Self {
        StreamInspectConnectNotes {
            client_addr: tcp_notes.local.unwrap(),
            server_addr: tcp_notes.next.unwrap(),
        }
    }
}

pub(crate) struct StreamInspectContext<SC: ServerConfig> {
    audit_handle: Arc<AuditHandle>,
    server_config: Arc<SC>,
    server_stats: ArcServerStats,
    server_quit_policy: Arc<ServerQuitPolicy>,
    idle_wheel: Arc<IdleWheel>,
    task_notes: StreamInspectTaskNotes,
    connect_notes: StreamInspectConnectNotes,
    inspection_depth: usize,

    max_idle_count: usize,
}

impl<SC: ServerConfig> Clone for StreamInspectContext<SC> {
    fn clone(&self) -> Self {
        StreamInspectContext {
            audit_handle: self.audit_handle.clone(),
            server_config: self.server_config.clone(),
            server_stats: self.server_stats.clone(),
            server_quit_policy: self.server_quit_policy.clone(),
            idle_wheel: self.idle_wheel.clone(),
            task_notes: self.task_notes.clone(),
            connect_notes: self.connect_notes,
            inspection_depth: self.inspection_depth,
            max_idle_count: self.max_idle_count,
        }
    }
}

impl<SC: ServerConfig> StreamInspectContext<SC> {
    pub(crate) fn new(
        audit_handle: Arc<AuditHandle>,
        server_config: Arc<SC>,
        server_stats: ArcServerStats,
        server_quit_policy: Arc<ServerQuitPolicy>,
        idle_wheel: Arc<IdleWheel>,
        task_notes: &ServerTaskNotes,
        tcp_notes: &TcpConnectTaskNotes,
    ) -> Self {
        let max_idle_count = task_notes
            .user_ctx()
            .and_then(|c| c.user().task_max_idle_count())
            .unwrap_or(server_config.task_max_idle_count());

        StreamInspectContext {
            audit_handle,
            server_config,
            server_stats,
            server_quit_policy,
            idle_wheel,
            task_notes: StreamInspectTaskNotes::from(task_notes),
            connect_notes: StreamInspectConnectNotes::from(tcp_notes),
            inspection_depth: 0,
            max_idle_count,
        }
    }

    #[inline]
    fn user(&self) -> Option<&User> {
        self.task_notes.user().map(|u| u.as_ref())
    }

    fn user_cloned(&self) -> Option<Arc<User>> {
        self.task_notes.user().cloned()
    }

    #[inline]
    fn raw_user_name(&self) -> Option<&Arc<str>> {
        self.task_notes.raw_username()
    }

    #[inline]
    pub(crate) fn server_task_id(&self) -> &Uuid {
        self.task_notes.task_id()
    }

    #[inline]
    fn server_force_quit(&self) -> bool {
        self.server_quit_policy.force_quit()
    }

    #[inline]
    fn server_offline(&self) -> bool {
        !self.server_stats.is_online()
    }

    #[inline]
    pub(crate) fn inspect_logger(&self) -> Option<&Logger> {
        self.audit_handle.inspect_logger()
    }

    #[inline]
    pub(crate) fn intercept_logger(&self) -> Option<&Logger> {
        self.audit_handle.intercept_logger()
    }

    pub(crate) fn idle_checker(&self) -> ServerIdleChecker {
        ServerIdleChecker::new(
            self.idle_wheel.clone(),
            self.user_cloned(),
            self.max_idle_count,
            self.server_quit_policy.clone(),
        )
    }

    pub(crate) fn protocol_inspector(
        &self,
        explicit_protocol: Option<MaybeProtocol>,
    ) -> ProtocolInspector {
        let mut inspector = ProtocolInspector::new(
            self.audit_handle.server_tcp_portmap(),
            self.audit_handle.client_tcp_portmap(),
        );
        if let Some(p) = explicit_protocol {
            inspector.push_protocol(p);
        }
        inspector
    }

    #[inline]
    pub(crate) fn current_inspection_depth(&self) -> usize {
        self.inspection_depth
    }

    #[inline]
    fn increase_inspection_depth(&mut self) {
        self.inspection_depth += 1;
    }

    #[inline]
    pub(crate) fn tls_interception(&self) -> Option<TlsInterceptionContext> {
        self.audit_handle.tls_interception()
    }

    pub(crate) fn user_site_tls_client(&self) -> Option<&OpensslClientConfig> {
        self.task_notes
            .user_ctx
            .as_ref()
            .and_then(|v| v.user_site.as_ref())
            .and_then(|v| v.tls_client())
    }

    fn log_uri_max_chars(&self) -> usize {
        self.task_notes
            .user_ctx
            .as_ref()
            .and_then(|cx| cx.user.log_uri_max_chars())
            .unwrap_or_else(|| self.audit_handle.log_uri_max_chars())
    }

    #[inline]
    fn h1_interception(&self) -> &H1InterceptionConfig {
        self.audit_handle.h1_interception()
    }

    fn h1_rsp_hdr_recv_timeout(&self) -> Duration {
        self.task_notes
            .user_ctx
            .as_ref()
            .and_then(|ctx| ctx.http_rsp_hdr_recv_timeout())
            .unwrap_or(self.h1_interception().rsp_head_recv_timeout)
    }

    #[inline]
    fn h2_inspect_action(&self, host: &Host) -> ProtocolInspectAction {
        match self.audit_handle.h2_inspect_policy.check(host) {
            (true, policy_action) => policy_action,
            (false, missing_policy_action) => missing_policy_action,
        }
    }

    #[inline]
    fn h2_interception(&self) -> &H2InterceptionConfig {
        self.audit_handle.h2_interception()
    }

    fn h2_rsp_hdr_recv_timeout(&self) -> Duration {
        self.task_notes
            .user_ctx
            .as_ref()
            .and_then(|ctx| ctx.http_rsp_hdr_recv_timeout())
            .unwrap_or(self.h2_interception().rsp_head_recv_timeout)
    }

    #[inline]
    fn websocket_inspect_action(&self, host: &Host) -> ProtocolInspectAction {
        match self.audit_handle.websocket_inspect_policy.check(host) {
            (true, policy_action) => policy_action,
            (false, missing_policy_action) => missing_policy_action,
        }
    }

    #[inline]
    fn smtp_inspect_action(&self, host: &Host) -> ProtocolInspectAction {
        match self.audit_handle.smtp_inspect_policy.check(host) {
            (true, policy_action) => policy_action,
            (false, missing_policy_action) => missing_policy_action,
        }
    }

    #[inline]
    fn smtp_interception(&self) -> &SmtpInterceptionConfig {
        self.audit_handle.smtp_interception()
    }

    #[inline]
    fn imap_inspect_action(&self, host: &Host) -> ProtocolInspectAction {
        match self.audit_handle.imap_inspect_policy.check(host) {
            (true, policy_action) => policy_action,
            (false, missing_policy_action) => missing_policy_action,
        }
    }

    #[inline]
    fn imap_interception(&self) -> &ImapInterceptionConfig {
        self.audit_handle.imap_interception()
    }

    fn belongs_to_blocked_user(&self) -> bool {
        self.task_notes
            .user_ctx
            .as_ref()
            .map(|cx| cx.user.is_blocked())
            .unwrap_or(false)
    }
}

pub(crate) enum StreamInspection<SC: ServerConfig> {
    End,
    StreamUnknown(stream::StreamInspectObject<SC>),
    StreamInspect(stream::StreamInspectObject<SC>),
    TlsModern(tls::TlsInterceptObject<SC>),
    #[cfg(feature = "vendored-tongsuo")]
    TlsTlcp(tls::TlsInterceptObject<SC>),
    StartTls(start_tls::StartTlsInterceptObject<SC>),
    H1(http::H1InterceptObject<SC>),
    H2(http::H2InterceptObject<SC>),
    Websocket(websocket::H1WebsocketInterceptObject<SC>),
    Smtp(smtp::SmtpInterceptObject<SC>),
    Imap(imap::ImapInterceptObject<SC>),
}

type BoxAsyncRead = Box<dyn AsyncRead + Send + Sync + Unpin + 'static>;
type BoxAsyncWrite = Box<dyn AsyncWrite + Send + Sync + Unpin + 'static>;
