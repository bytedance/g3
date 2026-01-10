/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use super::{MaybeProtocol, Protocol, ProtocolInspectError, ProtocolInspectState};

impl ProtocolInspectState {
    pub(crate) fn check_ssh_client_protocol_version_exchange(
        &mut self,
        data: &[u8],
    ) -> Result<Option<Protocol>, ProtocolInspectError> {
        // at least SSH-<x>.<y>-<z>\r\n
        const MINIMUM_DATA_LEN: usize = 11;

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
        self.exclude_other(MaybeProtocol::Stomp);
        self.exclude_other(MaybeProtocol::Smpp);
        self.exclude_other(MaybeProtocol::Rtmp);
        self.exclude_other(MaybeProtocol::BitTorrent);
        self.exclude_other(MaybeProtocol::Ldap);

        if &data[1..4] != b"SH-" {
            self.exclude_current();
            return Ok(None);
        }

        // exclude impossible protocols
        self.exclude_other(MaybeProtocol::Http);
        self.exclude_other(MaybeProtocol::Rtsp);

        // check ssh version
        let mut offset = 7;
        let protocol = match &data[4..6] {
            b"1." => match data[6] {
                b'9' => {
                    if data[7] == b'9' {
                        offset = 8;
                        Protocol::Ssh
                    } else {
                        Protocol::SshLegacy
                    }
                }
                b'0'..=b'8' => Protocol::SshLegacy,
                _ => {
                    self.exclude_current();
                    return Ok(None);
                }
            },
            b"2." => {
                if data[6] != b'0' {
                    self.exclude_current();
                    return Ok(None);
                }
                Protocol::Ssh
            }
            _ => {
                self.exclude_current();
                return Ok(None);
            }
        };

        if data[offset] != b'-' {
            self.exclude_current();
            return Ok(None);
        }

        // check trailing
        if data[data_len - 1] != b'\n' {
            return if data_len < 255 {
                Err(ProtocolInspectError::NeedMoreData(1))
            } else {
                self.exclude_current();
                Ok(None)
            };
        }
        if data[7] != b'9' {
            // no '\r' for SSH-1.99-
            if data[data_len - 2] != b'\r' {
                self.exclude_current();
                return Ok(None);
            }
        }

        Ok(Some(protocol))
    }

    pub(crate) fn check_ssh_server_protocol_version_exchange(
        &mut self,
        data: &[u8],
    ) -> Result<Option<Protocol>, ProtocolInspectError> {
        // at least SSH-<x>.<y>-<z>\r\n or SSH-1.99-<z>\n
        const MINIMUM_DATA_LEN: usize = 11;

        let data_len = data.len();
        if data_len < MINIMUM_DATA_LEN {
            return Err(ProtocolInspectError::NeedMoreData(
                MINIMUM_DATA_LEN - data_len,
            ));
        }

        if &data[0..4] != b"SSH-" {
            self.exclude_current();
            return Ok(None);
        }

        // exclude impossible protocols
        self.exclude_other(MaybeProtocol::Ftp);
        self.exclude_other(MaybeProtocol::Smtp);
        self.exclude_other(MaybeProtocol::Odmr);
        self.exclude_other(MaybeProtocol::Pop3);
        self.exclude_other(MaybeProtocol::Nntp);
        self.exclude_other(MaybeProtocol::Nnsp);
        self.exclude_other(MaybeProtocol::Imap);
        self.exclude_other(MaybeProtocol::Nats);
        self.exclude_other(MaybeProtocol::BitTorrent);

        // check ssh version
        let mut offset = 7;
        let protocol = match &data[4..6] {
            b"1." => match data[6] {
                b'9' => {
                    if data[7] == b'9' {
                        offset = 8;
                        Protocol::Ssh
                    } else {
                        Protocol::SshLegacy
                    }
                }
                b'0'..=b'8' => Protocol::SshLegacy,
                _ => {
                    self.exclude_current();
                    return Ok(None);
                }
            },
            b"2." => {
                if data[6] != b'0' {
                    self.exclude_current();
                    return Ok(None);
                }
                Protocol::Ssh
            }
            _ => {
                self.exclude_current();
                return Ok(None);
            }
        };

        if data[offset] != b'-' {
            self.exclude_current();
            return Ok(None);
        }

        // check trailing
        if data[data_len - 1] != b'\n' {
            return if data_len < 255 {
                Err(ProtocolInspectError::NeedMoreData(1))
            } else {
                self.exclude_current();
                Ok(None)
            };
        }
        if data[7] != b'9' {
            // no '\r' for SSH-1.99-
            if data[data_len - 2] != b'\r' {
                self.exclude_current();
                return Ok(None);
            }
        }

        Ok(Some(protocol))
    }
}
