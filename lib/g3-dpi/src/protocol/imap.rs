/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use super::{MaybeProtocol, Protocol, ProtocolInspectError, ProtocolInspectState};
use crate::ProtocolInspectionSizeLimit;

impl ProtocolInspectState {
    pub(crate) fn check_imap_server_greeting(
        &mut self,
        data: &[u8],
        size_limit: &ProtocolInspectionSizeLimit,
    ) -> Result<Option<Protocol>, ProtocolInspectError> {
        // at least * OK <M>\r\n
        const MINIMUM_DATA_LEN: usize = 6;
        // at least * PREAUTH\r\n
        const PREAUTH_MIN_DATA_LEN: usize = 11;

        let data_len = data.len();
        if data_len < MINIMUM_DATA_LEN {
            return Err(ProtocolInspectError::NeedMoreData(
                MINIMUM_DATA_LEN - data_len,
            ));
        }

        if data[0] != b'*' {
            // 0x2A
            self.exclude_current();
            return Ok(None);
        }

        // exclude impossible protocols
        self.exclude_other(MaybeProtocol::Ftp);
        self.exclude_other(MaybeProtocol::Ssh);
        self.exclude_other(MaybeProtocol::Smtp);
        self.exclude_other(MaybeProtocol::Odmr);
        self.exclude_other(MaybeProtocol::Pop3);
        self.exclude_other(MaybeProtocol::Nntp);
        self.exclude_other(MaybeProtocol::Nnsp);
        self.exclude_other(MaybeProtocol::Nats);
        self.exclude_other(MaybeProtocol::BitTorrent);

        if data[1] != b' ' {
            self.exclude_current();
            return Ok(None);
        }

        match data[2] {
            b'O' => {
                // "OK"
                if data[3] != b'K' {
                    self.exclude_current();
                    return Ok(None);
                }
            }
            b'P' => {
                // "PREAUTH"
                if data_len < PREAUTH_MIN_DATA_LEN {
                    return Err(ProtocolInspectError::NeedMoreData(
                        PREAUTH_MIN_DATA_LEN - data_len,
                    ));
                }
                if !data[2..].starts_with(b"PREAUTH") {
                    self.exclude_current();
                    return Ok(None);
                }
            }
            b'B' => {
                // "BYE"
                if data[3] != b'Y' || data[4] != b'E' {
                    self.exclude_current();
                    return Ok(None);
                }
            }
            _ => {
                self.exclude_current();
                return Ok(None);
            }
        }

        if data[data_len - 1] != b'\n' {
            let left = &data[4..];
            return if left.len() > size_limit.imap_server_greeting_msg {
                self.exclude_current();
                Ok(None)
            } else {
                Err(ProtocolInspectError::NeedMoreData(1))
            };
        }
        if data[data_len - 2] != b'\r' {
            self.exclude_current();
            return Ok(None);
        }

        Ok(Some(Protocol::Imap))
    }
}
