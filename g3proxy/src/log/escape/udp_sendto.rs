/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use slog::{Logger, slog_info};
use uuid::Uuid;

use g3_io_ext::{UdpCopyRemoteError, UdpRelayRemoteError};
use g3_slog_types::{LtDateTime, LtUpstreamAddr, LtUuid};
use g3_types::net::UpstreamAddr;

use crate::module::udp_connect::UdpConnectTaskNotes;
use crate::module::udp_relay::UdpRelayTaskNotes;

pub(crate) struct EscapeLogForUdpRelaySendto<'a> {
    pub(crate) task_id: &'a Uuid,
    pub(crate) udp_notes: &'a UdpRelayTaskNotes,
    pub(crate) remote_addr: &'a Option<UpstreamAddr>,
}

impl EscapeLogForUdpRelaySendto<'_> {
    pub(crate) fn log(&self, logger: &Logger, e: &UdpRelayRemoteError) {
        let (bind_addr, next_addr, reason) = match e {
            UdpRelayRemoteError::NoListenSocket => (None, None, "NoListenSocket"),
            UdpRelayRemoteError::RecvFailed(bind, _) => (Some(*bind), None, "RecvFailed"),
            UdpRelayRemoteError::SendFailed(bind, to, _) => (Some(*bind), Some(*to), "SendFailed"),
            UdpRelayRemoteError::BatchSendFailed(bind, _) => (Some(*bind), None, "BatchSendFailed"),
            UdpRelayRemoteError::InvalidPacket(bind, _) => (Some(*bind), None, "InvalidPacket"),
            UdpRelayRemoteError::AddressNotSupported => (None, None, "AddressNotSupported"),
            UdpRelayRemoteError::DomainNotResolved(_) => (None, None, "DomainNotResolved"),
            UdpRelayRemoteError::ForbiddenTargetIpAddress(to) => {
                (None, Some(*to), "ForbiddenTargetIpAddress")
            }
            UdpRelayRemoteError::RemoteSessionClosed(bind, to) => {
                (Some(*bind), Some(*to), "RemoteSessionClosed")
            }
            UdpRelayRemoteError::RemoteSessionError(bind, to, _) => {
                (Some(*bind), Some(*to), "RemoteSessionError")
            }
            UdpRelayRemoteError::InternalServerError(_) => (None, None, "InternalServerError"),
        };
        slog_info!(logger, "{}", e;
            "escape_type" => "UdpSendto",
            "task_id" => LtUuid(self.task_id),
            "upstream" => self.remote_addr.as_ref().map(LtUpstreamAddr),
            "next_bound_addr" => bind_addr,
            "next_peer_addr" => next_addr,
            "next_expire" => self.udp_notes.expire.as_ref().map(LtDateTime),
            "reason" => reason,
        )
    }
}

pub(crate) struct EscapeLogForUdpConnectSendTo<'a> {
    pub(crate) task_id: &'a Uuid,
    pub(crate) upstream: Option<&'a UpstreamAddr>,
    pub(crate) udp_notes: &'a UdpConnectTaskNotes,
}

impl EscapeLogForUdpConnectSendTo<'_> {
    pub(crate) fn log(&self, logger: &Logger, e: &UdpCopyRemoteError) {
        let reason = match e {
            UdpCopyRemoteError::SendFailed(_) => "SendFailed",
            UdpCopyRemoteError::RecvFailed(_) => "RecvFailed",
            UdpCopyRemoteError::InvalidPacket(_) => "InvalidPacket",
            UdpCopyRemoteError::RemoteSessionClosed => "RemoteSessionClosed",
            UdpCopyRemoteError::RemoteSessionError(_) => "RemoteSessionError",
            UdpCopyRemoteError::InternalServerError(_) => "InternalServerError",
        };
        slog_info!(logger, "{}", e;
            "escape_type" => "UdpSendto",
            "task_id" => LtUuid(self.task_id),
            "upstream" => self.upstream.map(LtUpstreamAddr),
            "next_bound_addr" => self.udp_notes.local,
            "next_peer_addr" => self.udp_notes.next,
            "next_expire" => self.udp_notes.expire.as_ref().map(LtDateTime),
            "reason" => reason,
        )
    }
}
