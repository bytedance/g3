/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use super::{MaybeProtocol, Protocol, ProtocolInspectError, ProtocolInspectState};

impl ProtocolInspectState {
    pub(crate) fn check_pop3_server_greeting(
        &mut self,
        data: &[u8],
    ) -> Result<Option<Protocol>, ProtocolInspectError> {
        // at least +OK <M>\r\n
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

        if data[0] != b'+' {
            // 0x2B
            self.exclude_current();
            return Ok(None);
        }

        // exclude impossible protocols
        self.exclude_other(MaybeProtocol::Ftp);
        self.exclude_other(MaybeProtocol::Ssh);
        self.exclude_other(MaybeProtocol::Smtp);
        self.exclude_other(MaybeProtocol::Odmr);
        self.exclude_other(MaybeProtocol::Nntp);
        self.exclude_other(MaybeProtocol::Nnsp);
        self.exclude_other(MaybeProtocol::Imap);
        self.exclude_other(MaybeProtocol::Nats);
        self.exclude_other(MaybeProtocol::BitTorrent);

        if &data[1..3] != b"OK" {
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

        Ok(Some(Protocol::Pop3))
    }
}
