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
    pub(crate) fn check_stomp_client_connect_request(
        &mut self,
        data: &[u8],
    ) -> Result<Option<Protocol>, ProtocolInspectError> {
        // at least "CONNECT\n\n\0" or "STOMP\naccept-version:1.2\nhost:<xxx>\n\0"
        const MINIMUM_DATA_LEN: usize = 10;

        let data_len = data.len();
        if data_len < MINIMUM_DATA_LEN {
            return Err(ProtocolInspectError::NeedMoreData(
                MINIMUM_DATA_LEN - data_len,
            ));
        }

        match data[0] {
            // 0x43
            b'C' => self.check_stomp_connect_method(data),
            // 0x53
            b'S' => self.check_stomp_stomp_method(data),
            _ => {
                self.exclude_current();
                Ok(None)
            }
        }
    }

    fn check_stomp_connect_method(
        &mut self,
        data: &[u8],
    ) -> Result<Option<Protocol>, ProtocolInspectError> {
        // at least "CONNECT\n\n\0"
        const MINIMUM_LEN_AFTER_METHOD: usize = 2;

        // exclude impossible protocols
        self.exclude_other(MaybeProtocol::Ssl);
        self.exclude_other(MaybeProtocol::Ssh);
        self.exclude_other(MaybeProtocol::Rtsp);
        self.exclude_other(MaybeProtocol::Mqtt);
        self.exclude_other(MaybeProtocol::Rtmp);
        self.exclude_other(MaybeProtocol::BitTorrent);

        if &data[1..7] != b"ONNECT" {
            self.exclude_current();
            return Ok(None);
        }

        let offset: usize = match data[7] {
            b'\r' => {
                self.exclude_other(MaybeProtocol::Http);
                if data[8] != b'\n' {
                    self.exclude_current();
                    return Ok(None);
                }
                9
            }
            b'\n' => {
                self.exclude_other(MaybeProtocol::Http);
                8
            }
            _ => {
                self.exclude_current();
                return Ok(None);
            }
        };

        let data_len = data.len();
        if offset + MINIMUM_LEN_AFTER_METHOD > data_len {
            return Err(ProtocolInspectError::NeedMoreData(
                offset + MINIMUM_LEN_AFTER_METHOD - data_len,
            ));
        }

        // TODO check header and ending '\0'

        Ok(Some(Protocol::Stomp))
    }

    fn check_stomp_stomp_method(
        &mut self,
        data: &[u8],
    ) -> Result<Option<Protocol>, ProtocolInspectError> {
        // at least "STOMP\naccept-version:1.2\nhost:<xxx>\n\0"
        const MINIMUM_LEN_AFTER_METHOD: usize = 26;

        // exclude impossible protocols
        self.exclude_other(MaybeProtocol::Ssl);
        self.exclude_other(MaybeProtocol::Ssh);
        self.exclude_other(MaybeProtocol::Mqtt);
        self.exclude_other(MaybeProtocol::Rtmp);
        self.exclude_other(MaybeProtocol::BitTorrent);

        if &data[1..5] != b"TOMP" {
            self.exclude_current();
            return Ok(None);
        }
        self.exclude_other(MaybeProtocol::Http);
        self.exclude_other(MaybeProtocol::Rtsp);

        let offset: usize = match data[5] {
            b'\r' => {
                if data[6] != b'\n' {
                    self.exclude_current();
                    return Ok(None);
                }
                7
            }
            b'\n' => 6,
            _ => {
                self.exclude_current();
                return Ok(None);
            }
        };

        let data_len = data.len();
        if offset + MINIMUM_LEN_AFTER_METHOD > data_len {
            return Err(ProtocolInspectError::NeedMoreData(
                offset + MINIMUM_LEN_AFTER_METHOD - data_len,
            ));
        }

        // TODO check header and ending '\0'

        Ok(Some(Protocol::Stomp))
    }
}
