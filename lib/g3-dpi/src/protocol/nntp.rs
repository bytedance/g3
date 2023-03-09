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
    pub(crate) fn check_nntp_server_greeting(
        &mut self,
        data: &[u8],
    ) -> Result<Option<Protocol>, ProtocolInspectError> {
        // at least 20X <M>\r\n
        const MINIMUM_DATA_LEN: usize = 5;
        const MAXIMUM_DATA_LEN: usize = 512;

        let data_len = data.len();
        if data_len < MINIMUM_DATA_LEN {
            return Err(ProtocolInspectError::NeedMoreData(
                MINIMUM_DATA_LEN - data_len,
            ));
        }
        if data_len > MAXIMUM_DATA_LEN {
            self.exclude_current();
            return Ok(None);
        }

        if data[0] != b'2' {
            // 0x32
            self.exclude_current();
            return Ok(None);
        }

        // exclude impossible protocols
        self.exclude_other(MaybeProtocol::Ssh);
        self.exclude_other(MaybeProtocol::Nntp);
        self.exclude_other(MaybeProtocol::Imap);
        self.exclude_other(MaybeProtocol::Nats);
        self.exclude_other(MaybeProtocol::BitTorrent);

        if data[1] != b'0' {
            self.exclude_current();
            return Ok(None);
        }

        // exclude impossible protocols
        self.exclude_other(MaybeProtocol::Ftp);
        self.exclude_other(MaybeProtocol::Smtp);

        if !matches!(data[2], b'0' | b'1') {
            self.exclude_current();
            return Ok(None);
        }

        if data[data_len - 1] != b'\n' {
            return Err(ProtocolInspectError::NeedMoreData(1));
        }
        if data[data_len - 2] != b'\r' {
            self.exclude_current();
            return Ok(None);
        }

        Ok(Some(Protocol::Nntp))
    }
}
