/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use super::{MaybeProtocol, Protocol, ProtocolInspectError, ProtocolInspectState};

impl ProtocolInspectState {
    pub(crate) fn check_rtsp_client_setup_request(
        &mut self,
        data: &[u8],
    ) -> Result<Option<Protocol>, ProtocolInspectError> {
        // at least SETUP rtsp://<x> RTSP/<N>.0\r\n
        const MINIMUM_DATA_LEN: usize = 25;

        let data_len = data.len();
        if data_len < MINIMUM_DATA_LEN {
            return Err(ProtocolInspectError::NeedMoreData(
                MINIMUM_DATA_LEN - data_len,
            ));
        }

        if data[0] != b'S' {
            // 0x53
            self.exclude_current();
            return Ok(None);
        }

        // exclude impossible protocols
        self.exclude_other(MaybeProtocol::Ssl);
        self.exclude_other(MaybeProtocol::Mqtt);
        self.exclude_other(MaybeProtocol::Smpp);
        self.exclude_other(MaybeProtocol::Rtmp);
        self.exclude_other(MaybeProtocol::BitTorrent);

        if !data.starts_with(b"SETUP rtsp://") {
            self.exclude_current();
            return Ok(None);
        }

        self.exclude_other(MaybeProtocol::Ssh);
        self.exclude_other(MaybeProtocol::Http);
        self.exclude_other(MaybeProtocol::Stomp);

        // seems there's no need to do more check

        Ok(Some(Protocol::Rtsp))
    }
}
