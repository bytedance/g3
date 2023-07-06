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
    pub(crate) fn check_rtmp_client_handshake(
        &mut self,
        data: &[u8],
    ) -> Result<Option<Protocol>, ProtocolInspectError> {
        const RTMP_VERSION: u8 = 3;
        const RTMP_HANDSHAKE_C0_LEN: usize = 1;
        const RTMP_HANDSHAKE_C1_LEN: usize = 1536;
        const RTMP_HANDSHAKE_MINIMAL_LEN: usize = 9;
        const RTMP_HANDSHAKE_LEN: usize = RTMP_HANDSHAKE_C0_LEN + RTMP_HANDSHAKE_C1_LEN;

        let data_len = data.len();
        if data_len < RTMP_HANDSHAKE_MINIMAL_LEN {
            return Err(ProtocolInspectError::NeedMoreData(
                RTMP_HANDSHAKE_LEN - data_len,
            ));
        }

        if data[0] != RTMP_VERSION {
            // 0x03
            self.exclude_current();
            return Ok(None);
        }

        // exclude impossible protocols
        self.exclude_other(MaybeProtocol::Ssh);
        self.exclude_other(MaybeProtocol::Http);
        self.exclude_other(MaybeProtocol::Ssl);
        self.exclude_other(MaybeProtocol::Rtsp);
        self.exclude_other(MaybeProtocol::Mqtt);
        self.exclude_other(MaybeProtocol::Stomp);
        self.exclude_other(MaybeProtocol::BitTorrent);

        if &data[5..9] != b"\x00\x00\x00\x00" {
            self.exclude_current();
            return Ok(None);
        }

        if data_len < RTMP_HANDSHAKE_LEN {
            return Err(ProtocolInspectError::NeedMoreData(
                RTMP_HANDSHAKE_LEN - data_len,
            ));
        }

        Ok(Some(Protocol::Rtmp))
    }
}
