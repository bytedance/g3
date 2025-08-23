/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use g3_types::net::Host;

use super::{MaybeProtocol, Protocol, ProtocolInspectError, ProtocolInspectState};

const REPLY_LINE_MAX_SIZE: usize = 512;

impl ProtocolInspectState {
    pub(crate) fn check_smtp_server_greeting(
        &mut self,
        data: &[u8],
    ) -> Result<Option<Protocol>, ProtocolInspectError> {
        // at least XYZ <address>\r\n
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
                self.exclude_other(MaybeProtocol::Nnsp);

                if data[2] == b'0' {
                    // may be FTP
                    return self.check_smtp_after_code(data);
                }
            }
            b'5' => {
                // 0x35
                self.smtp_exclude_by_byte0();
                self.exclude_other(MaybeProtocol::Ftp);
                self.exclude_other(MaybeProtocol::Nntp);
                self.exclude_other(MaybeProtocol::Nnsp);

                if &data[0..3] == b"554" {
                    return self.check_smtp_after_code(data);
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
    ) -> Result<Option<Protocol>, ProtocolInspectError> {
        if !matches!(data[3], b' ' | b'-') {
            self.exclude_current();
            return Ok(None);
        }

        let left = &data[4..];
        if let Some(p) = memchr::memchr(b'\n', left) {
            if p < 1 {
                // at lease "<addr>\n"
                self.exclude_current();
                return Ok(None);
            }
            if left[p - 1] != b'\r' {
                self.exclude_current();
                return Ok(None);
            }

            let end_host = memchr::memchr(b' ', left).unwrap_or(p - 1);
            if Host::parse_smtp_host_address(&left[..end_host]).is_some() {
                Ok(Some(Protocol::Smtp))
            } else {
                Ok(None)
            }
        } else if left.len() > REPLY_LINE_MAX_SIZE {
            self.exclude_current();
            Ok(None)
        } else {
            Err(ProtocolInspectError::NeedMoreData(1))
        }
    }
}
