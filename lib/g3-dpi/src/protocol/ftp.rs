/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use super::{MaybeProtocol, Protocol, ProtocolInspectError, ProtocolInspectState};
use crate::ProtocolInspectionSizeLimit;

impl ProtocolInspectState {
    pub(crate) fn check_ftp_server_greeting(
        &mut self,
        data: &[u8],
        size_limit: &ProtocolInspectionSizeLimit,
    ) -> Result<Option<Protocol>, ProtocolInspectError> {
        // at least XYZ <M>\n
        const MINIMUM_DATA_LEN: usize = 6;

        let data_len = data.len();
        if data_len < MINIMUM_DATA_LEN {
            return Err(ProtocolInspectError::NeedMoreData(
                MINIMUM_DATA_LEN - data_len,
            ));
        }

        match data[0] {
            b'1' => {
                // 0x31
                self.ftp_exclude_by_byte0();
                self.exclude_other(MaybeProtocol::Smtp);
                self.exclude_other(MaybeProtocol::Odmr);
                self.exclude_other(MaybeProtocol::Nntp);
                self.exclude_other(MaybeProtocol::Nnsp);

                if &data[0..3] == b"120" {
                    return self.check_ftp_after_code(data, size_limit);
                }
            }
            b'2' => {
                // 0x32
                self.ftp_exclude_by_byte0();

                if data[1] != b'2' {
                    self.exclude_current();
                    return Ok(None);
                }
                self.exclude_other(MaybeProtocol::Nntp);
                self.exclude_other(MaybeProtocol::Nnsp);

                if data[2] == b'0' {
                    // may be SMTP
                    return self.check_ftp_after_code(data, size_limit);
                }
            }
            b'4' => {
                // 0x34
                self.ftp_exclude_by_byte0();
                self.exclude_other(MaybeProtocol::Smtp);
                self.exclude_other(MaybeProtocol::Odmr);
                self.exclude_other(MaybeProtocol::Nntp);
                self.exclude_other(MaybeProtocol::Nnsp);

                if &data[0..3] == b"421" {
                    return self.check_ftp_after_code(data, size_limit);
                }
            }
            _ => {}
        }

        self.exclude_current();
        Ok(None)
    }

    fn ftp_exclude_by_byte0(&mut self) {
        self.exclude_other(MaybeProtocol::Ssh);
        self.exclude_other(MaybeProtocol::Pop3);
        self.exclude_other(MaybeProtocol::Imap);
        self.exclude_other(MaybeProtocol::Nats);
        self.exclude_other(MaybeProtocol::BitTorrent);
    }

    fn check_ftp_after_code(
        &mut self,
        data: &[u8],
        size_limit: &ProtocolInspectionSizeLimit,
    ) -> Result<Option<Protocol>, ProtocolInspectError> {
        if !matches!(data[3], b' ' | b'-') {
            self.exclude_current();
            return Ok(None);
        }

        let left = &data[4..];
        match memchr::memchr(b'\n', left) {
            Some(_) => Ok(Some(Protocol::FtpControl)),
            None => {
                if left.len() > size_limit.ftp_server_greeting_msg {
                    self.exclude_current();
                    Ok(None)
                } else {
                    Err(ProtocolInspectError::NeedMoreData(1))
                }
            }
        }
    }
}
