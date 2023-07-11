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

use slog::{slog_info, Logger};
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
            "upstream" => self.udp_notes.upstream.as_ref().map(LtUpstreamAddr),
            "next_bound_addr" => self.udp_notes.local,
            "next_peer_addr" => self.udp_notes.next,
            "next_expire" => self.udp_notes.expire.as_ref().map(LtDateTime),
            "reason" => reason,
        )
    }
}
