/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use super::{MaybeProtocol, Protocol, ProtocolInspectError, ProtocolInspectState};

impl ProtocolInspectState {
    pub(crate) fn check_mqtt_client_connect_request(
        &mut self,
        data: &[u8],
    ) -> Result<Option<Protocol>, ProtocolInspectError> {
        // at least FixedHeader, Protocol Name, Protocol Level, Connect Flags, and Keep Alive
        const MINIMUM_DATA_LEN: usize = 12;

        let data_len = data.len();
        if data_len < MINIMUM_DATA_LEN {
            return Err(ProtocolInspectError::NeedMoreData(
                MINIMUM_DATA_LEN - data_len,
            ));
        }

        if data[0] != 0x10 {
            self.exclude_current();
            return Ok(None);
        }

        // exclude impossible protocols
        self.exclude_other(MaybeProtocol::Ssl);
        self.exclude_other(MaybeProtocol::Ssh);
        self.exclude_other(MaybeProtocol::Http);
        self.exclude_other(MaybeProtocol::Rtsp);
        self.exclude_other(MaybeProtocol::Stomp);
        self.exclude_other(MaybeProtocol::Smpp);
        self.exclude_other(MaybeProtocol::Rtmp);
        self.exclude_other(MaybeProtocol::BitTorrent);

        let remaining_len = data[1] as usize;
        if remaining_len + 2 < MINIMUM_DATA_LEN {
            self.exclude_current();
            return Ok(None);
        }

        if &data[2..8] != b"\x00\x04MQTT" {
            self.exclude_current();
            return Ok(None);
        }

        let protocol_level = data[8];
        match protocol_level {
            0x04 => {}
            0x05 => {}
            _ => {
                self.exclude_current();
                return Ok(None);
            }
        }

        Ok(Some(Protocol::Mqtt))
    }
}
