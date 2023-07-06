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

use super::{MaybeProtocol, Protocol, ProtocolInspectError, ProtocolInspectState};

impl ProtocolInspectState {
    pub(crate) fn check_bittorrent_handshake(
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
        self.exclude_other(MaybeProtocol::Pop3);
        self.exclude_other(MaybeProtocol::Imap);
        self.exclude_other(MaybeProtocol::Http);
        self.exclude_other(MaybeProtocol::Ssl);
        self.exclude_other(MaybeProtocol::Rtsp);
        self.exclude_other(MaybeProtocol::Mqtt);
        self.exclude_other(MaybeProtocol::Stomp);
        self.exclude_other(MaybeProtocol::Rtmp);
        self.exclude_other(MaybeProtocol::Nats);

        if data[1..].starts_with(b"BitTorrent protocol") {
            Ok(Some(Protocol::BitTorrent))
        } else {
            self.exclude_current();
            Ok(None)
        }
    }
}
