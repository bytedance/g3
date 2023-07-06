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

use bytes::Buf;

use super::{MaybeProtocol, Protocol, ProtocolInspectError, ProtocolInspectState};

#[allow(dead_code)]
#[repr(u8)]
enum TlsContentType {
    Invalid = 0,
    ChangeCipherSpec = 20,
    Alert = 21,
    Handshake = 22, // 0x16
    ApplicationData = 23,
    Heartbeat = 24,
}

#[allow(dead_code)]
#[repr(u8)]
enum TlsHandshakeType {
    HelloRequestReserved = 0,
    ClientHello = 1,
    ServerHell0 = 2,
    HelloVerifyRequestReserved = 3,
    // there are more that we don't need
}

impl ProtocolInspectState {
    pub(crate) fn check_ssl_client_hello(
        &mut self,
        data: &[u8],
    ) -> Result<Option<Protocol>, ProtocolInspectError> {
        const SSL_HDR_LEN: usize = 5;
        const SSL_HANDSHAKE_HDR_LEN: usize = 4;
        const SSL_HANDSHAKE_VERSION_LEN: usize = 2;
        const MINIMUM_DATA_LEN: usize =
            SSL_HDR_LEN + SSL_HANDSHAKE_HDR_LEN + SSL_HANDSHAKE_VERSION_LEN;

        let data_len = data.len();

        /*
         * check ssh hdr
         *
         * struct ssl_hdr {
         *     __u8   type;
         *     __be16 legacy_record_version;
         *     __be16 len;
         * } __attribute_packed__;
         */
        if data_len < MINIMUM_DATA_LEN {
            return Err(ProtocolInspectError::NeedMoreData(
                MINIMUM_DATA_LEN - data_len,
            ));
        }

        if data[0] != TlsContentType::Handshake as u8 {
            self.exclude_current();
            return Ok(None);
        }

        // exclude impossible protocols
        self.exclude_other(MaybeProtocol::Ssh);
        self.exclude_other(MaybeProtocol::Http);
        self.exclude_other(MaybeProtocol::Rtsp);
        self.exclude_other(MaybeProtocol::Mqtt);
        self.exclude_other(MaybeProtocol::Stomp);
        self.exclude_other(MaybeProtocol::Rtmp);
        self.exclude_other(MaybeProtocol::BitTorrent);

        let mut buf = &data[1..];
        let legacy_record_version = buf.get_u16();
        let _legacy_protocol = match legacy_record_version {
            0x0101 => Protocol::TlsTlcp,
            0x0200 | 0x0300 => Protocol::SslLegacy,
            0x0301 | 0x0302 => Protocol::TlsLegacy,
            0x0303 | 0x0304 => Protocol::TlsModern,
            _ => {
                self.exclude_current();
                return Ok(None);
            }
        };

        let ssl_payload_len = buf.get_u16() as usize;
        if ssl_payload_len < data_len - SSL_HDR_LEN {
            self.exclude_current();
            return Ok(None);
        }

        /*
         * check ssh handshake hdr
         * struct ssl_handshake_hdr {
         *     __be32 hdr;
         * } __attribute_packed__;
         */
        let buf = &data[SSL_HDR_LEN..];
        if buf[0] != TlsHandshakeType::ClientHello as u8 {
            self.exclude_current();
            return Ok(None);
        }
        let handshake_payload_len = u32::from_be_bytes([0u8, buf[1], buf[2], buf[3]]) as usize;
        if handshake_payload_len + SSL_HANDSHAKE_HDR_LEN != ssl_payload_len {
            self.exclude_current();
            return Ok(None);
        }

        let protocol = match (buf[4], buf[5]) {
            (0x01, 0x01) => Protocol::TlsTlcp,
            (0x02, 0x00) | (0x03, 0x00) => Protocol::SslLegacy,
            (0x03, 0x01) | (0x03, 0x02) => Protocol::TlsLegacy,
            (0x03, 0x03) | (0x03, 0x04) => Protocol::TlsModern,
            _ => {
                self.exclude_current();
                return Ok(None);
            }
        };

        Ok(Some(protocol))
    }
}
