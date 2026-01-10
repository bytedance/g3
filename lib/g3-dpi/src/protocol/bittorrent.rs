/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use super::{MaybeProtocol, Protocol, ProtocolInspectError, ProtocolInspectState};

impl ProtocolInspectState {
    pub(crate) fn check_bittorrent_tcp_handshake(
        &mut self,
        data: &[u8],
    ) -> Result<Option<Protocol>, ProtocolInspectError> {
        // at least 0x13 BitTorrent <SP> protocol <Extension, 8 bytes> <sha1 hash of info, 20 bytes> <peer id, 20bytes>
        const BT_HANDSHAKE_SIZE: usize = 68;

        let data_len = data.len();
        if data_len < BT_HANDSHAKE_SIZE {
            return Err(ProtocolInspectError::NeedMoreData(
                BT_HANDSHAKE_SIZE - data_len,
            ));
        }

        if data[0] != 0x13 {
            self.exclude_current();
            return Ok(None);
        }

        // exclude impossible protocols
        self.exclude_other(MaybeProtocol::Ftp);
        self.exclude_other(MaybeProtocol::Ssh);
        self.exclude_other(MaybeProtocol::Smtp);
        self.exclude_other(MaybeProtocol::Odmr);
        self.exclude_other(MaybeProtocol::Pop3);
        self.exclude_other(MaybeProtocol::Imap);
        self.exclude_other(MaybeProtocol::Http);
        self.exclude_other(MaybeProtocol::Ssl);
        self.exclude_other(MaybeProtocol::Rtsp);
        self.exclude_other(MaybeProtocol::Mqtt);
        self.exclude_other(MaybeProtocol::Stomp);
        self.exclude_other(MaybeProtocol::Smpp);
        self.exclude_other(MaybeProtocol::Rtmp);
        self.exclude_other(MaybeProtocol::Nats);
        self.exclude_other(MaybeProtocol::Ldap);

        if data[1..].starts_with(b"BitTorrent protocol") {
            Ok(Some(Protocol::BitTorrentOverTcp))
        } else {
            self.exclude_current();
            Ok(None)
        }
    }
}
