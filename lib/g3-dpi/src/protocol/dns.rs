/*
 * Copyright 2024 ByteDance and/or its affiliates.
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

use super::{Protocol, ProtocolInspectError, ProtocolInspectState};

const DNS_MESSAGE_HEADER_LEN: usize = 12;

impl ProtocolInspectState {
    pub(crate) fn check_dns_tcp_request_message(
        &mut self,
        data: &[u8],
    ) -> Result<Option<Protocol>, ProtocolInspectError> {
        // 2 Byte Length + Header
        const MINIMUM_DATA_LEN: usize = 2 + DNS_MESSAGE_HEADER_LEN;

        let data_len = data.len();
        if data_len < MINIMUM_DATA_LEN {
            return Err(ProtocolInspectError::NeedMoreData(
                MINIMUM_DATA_LEN - data_len,
            ));
        }

        let message_len = u16::from_be_bytes([data[0], data[1]]) as usize;
        if message_len < DNS_MESSAGE_HEADER_LEN {
            self.exclude_current();
            return Ok(None);
        }

        if self.check_dns_request_message_header(&data[2..]).is_err() {
            self.exclude_current();
            return Ok(None);
        }

        Ok(Some(Protocol::Dns))
    }

    fn check_dns_request_message_header(&mut self, hdr: &[u8]) -> Result<(), ()> {
        if hdr[2] & 0b1000_0000 != 0 {
            // QR bit is not query
            return Err(());
        }

        if hdr[6..DNS_MESSAGE_HEADER_LEN] != [0x00, 0x00, 0x00, 0x00, 0x00, 0x00] {
            // there should be no any an / ns / ar count
            return Err(());
        }

        Ok(())
    }
}
