/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use super::{MaybeProtocol, Protocol, ProtocolInspectError, ProtocolInspectState};

const SMPP_BIND_TRANSMITTER: u32 = 0x00000002;
const SMPP_BIND_RECEIVER: u32 = 0x00000001;
const SMPP_BIND_TRANSCEIVER: u32 = 0x00000009;
const SMPP_OUTBIND: u32 = 0x0000000B;

const SMPP_BIND_MIN_BODY: usize = 7;
const SMPP_BIND_MAX_BODY: usize = 16 + 9 + 13 + 1 + 1 + 1 + 41;
const SMPP_OUTBIND_MIN_BODY: usize = 2;
const SMPP_OUTBIND_MAX_BODY: usize = 16 + 9;

impl ProtocolInspectState {
    pub(crate) fn check_smpp_session_request(
        &mut self,
        data: &[u8],
    ) -> Result<Option<Protocol>, ProtocolInspectError> {
        const SMPP_SESSION_REQUEST_HEADER_LEN: usize = 16;

        let data_len = data.len();
        if data_len < SMPP_SESSION_REQUEST_HEADER_LEN {
            return Err(ProtocolInspectError::NeedMoreData(
                SMPP_SESSION_REQUEST_HEADER_LEN - data_len,
            ));
        }

        if data[0] != 0x00 {
            // the request is too small so the first byte should always be 0
            self.exclude_current();
            return Ok(None);
        }
        self.exclude_other(MaybeProtocol::Ssh);
        self.exclude_other(MaybeProtocol::Http);
        self.exclude_other(MaybeProtocol::Ssl);
        self.exclude_other(MaybeProtocol::Rtsp);
        self.exclude_other(MaybeProtocol::Mqtt);
        self.exclude_other(MaybeProtocol::Stomp);
        self.exclude_other(MaybeProtocol::Rtmp);
        self.exclude_other(MaybeProtocol::BitTorrent);

        let cmd_len = u32::from_be_bytes([data[0], data[1], data[2], data[3]]) as usize;
        if cmd_len < SMPP_SESSION_REQUEST_HEADER_LEN {
            self.exclude_current();
            return Ok(None);
        }

        let cmd_id = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
        let (min_body, max_body) = match cmd_id {
            SMPP_BIND_TRANSMITTER | SMPP_BIND_RECEIVER | SMPP_BIND_TRANSCEIVER => {
                // ESME to MC
                (SMPP_BIND_MIN_BODY, SMPP_BIND_MAX_BODY)
            }
            SMPP_OUTBIND => {
                // MC to ESME
                (SMPP_OUTBIND_MIN_BODY, SMPP_OUTBIND_MAX_BODY)
            }
            _ => {
                self.exclude_current();
                return Ok(None);
            }
        };
        let body_len = cmd_len - SMPP_SESSION_REQUEST_HEADER_LEN;
        if body_len < min_body || body_len > max_body {
            self.exclude_current();
            return Ok(None);
        }

        let cmd_status = u32::from_be_bytes([data[8], data[9], data[10], data[11]]);
        if cmd_status != 0 {
            self.exclude_current();
            return Ok(None);
        }

        Ok(Some(Protocol::Smpp))
    }
}
