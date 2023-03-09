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
use crate::ProtocolInspectionSizeLimit;

impl ProtocolInspectState {
    pub(crate) fn check_smtp_server_greeting(
        &mut self,
        data: &[u8],
        size_limit: &ProtocolInspectionSizeLimit,
    ) -> Result<Option<Protocol>, ProtocolInspectError> {
        // at least XYZ <M>\r\n
        const MINIMUM_DATA_LEN: usize = 7;

        let data_len = data.len();
        if data_len < MINIMUM_DATA_LEN {
            return Err(ProtocolInspectError::NeedMoreData(
                MINIMUM_DATA_LEN - data_len,
            ));
        }

        match data[0] {
            b'2' => {
                // 0x32
                self.smtp_exclude_by_byte0();

                if data[1] != b'2' {
                    self.exclude_current();
                    return Ok(None);
                }
                self.exclude_other(MaybeProtocol::Nntp);

                if data[2] == b'0' {
                    // may be FTP
                    return self.check_smtp_after_code(data, size_limit);
                }
            }
            b'5' => {
                // 0x35
                self.smtp_exclude_by_byte0();
                self.exclude_other(MaybeProtocol::Ftp);
                self.exclude_other(MaybeProtocol::Nntp);

                if &data[0..3] == b"554" {
                    return self.check_smtp_after_code(data, size_limit);
                }
            }
            _ => {}
        }

        self.exclude_current();
        Ok(None)
    }

    fn smtp_exclude_by_byte0(&mut self) {
        self.exclude_other(MaybeProtocol::Ssh);
        self.exclude_other(MaybeProtocol::Pop3);
        self.exclude_other(MaybeProtocol::Imap);
        self.exclude_other(MaybeProtocol::Nats);
        self.exclude_other(MaybeProtocol::BitTorrent);
    }

    fn check_smtp_after_code(
        &mut self,
        data: &[u8],
        size_limit: &ProtocolInspectionSizeLimit,
    ) -> Result<Option<Protocol>, ProtocolInspectError> {
        if !matches!(data[3], b' ' | b'-') {
            self.exclude_current();
            return Ok(None);
        }

        let left = &data[4..];
        if let Some(p) = memchr::memchr(b'\n', left) {
            if p > 1 && left[p - 1] == b'\r' {
                return Ok(Some(Protocol::Smtp));
            }
        }

        if left.len() > size_limit.smtp_server_greeting_msg {
            self.exclude_current();
            Ok(None)
        } else {
            Err(ProtocolInspectError::NeedMoreData(1))
        }
    }
}
