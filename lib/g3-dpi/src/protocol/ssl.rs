/*
 * SPDX-License-Identifier: Apache-2.0
 * Copyright 2023-2025 ByteDance and/or its affiliates.
 */

use super::{MaybeProtocol, Protocol, ProtocolInspectError, ProtocolInspectState};
use crate::parser::tls::{ContentType, HandshakeType};

const SSL_HDR_LEN: usize = 5;
const SSL_HANDSHAKE_HDR_LEN: usize = 4;
const SSL_HANDSHAKE_VERSION_LEN: usize = 2;

impl ProtocolInspectState {
    pub(crate) fn check_ssl_client_hello(
        &mut self,
        data: &[u8],
    ) -> Result<Option<Protocol>, ProtocolInspectError> {
        let data_len = data.len();

        /*
         * check ssl hdr
         *
         * struct ssl_hdr {
         *     __u8   type;
         *     __be16 legacy_record_version;
         *     __be16 len;
         * } __attribute_packed__;
         */
        if data_len < SSL_HDR_LEN {
            return Err(ProtocolInspectError::NeedMoreData(SSL_HDR_LEN - data_len));
        }

        if data[0] != ContentType::Handshake as u8 {
            self.exclude_current();
            return Ok(None);
        }

        // exclude impossible protocols
        self.exclude_other(MaybeProtocol::Ssh);
        self.exclude_other(MaybeProtocol::Http);
        self.exclude_other(MaybeProtocol::Rtsp);
        self.exclude_other(MaybeProtocol::Mqtt);
        self.exclude_other(MaybeProtocol::Stomp);
        self.exclude_other(MaybeProtocol::Smpp);
        self.exclude_other(MaybeProtocol::Rtmp);
        self.exclude_other(MaybeProtocol::BitTorrent);

        if check_legacy_version(data[1], data[2]).is_err() {
            self.exclude_current();
            return Ok(None);
        }
        let Ok(fragment_len) = check_fragment_len(data[3], data[4]) else {
            self.exclude_current();
            return Ok(None);
        };

        if data_len < SSL_HDR_LEN + SSL_HANDSHAKE_HDR_LEN + SSL_HANDSHAKE_VERSION_LEN {
            return Err(ProtocolInspectError::NeedMoreData(
                SSL_HDR_LEN + SSL_HANDSHAKE_HDR_LEN + SSL_HANDSHAKE_VERSION_LEN - data_len,
            ));
        }
        if fragment_len >= SSL_HANDSHAKE_HDR_LEN + SSL_HANDSHAKE_VERSION_LEN {
            // seen full Handshake Message header in the first record
            return self
                .check_ssl_client_hello_full_handshake_header(fragment_len, &data[SSL_HDR_LEN..]);
        }

        let mut offset = SSL_HDR_LEN;
        let left = &data[offset..];
        let mut msg_hdr_nw = 0usize;
        let mut msg_hdr = [0u8; SSL_HANDSHAKE_HDR_LEN + SSL_HANDSHAKE_VERSION_LEN];
        unsafe {
            std::ptr::copy_nonoverlapping(left.as_ptr(), msg_hdr.as_mut_ptr(), fragment_len);
        }
        offset += fragment_len;
        msg_hdr_nw += fragment_len;

        loop {
            let left = &data[offset..];
            if left.is_empty() {
                return Err(ProtocolInspectError::NeedMoreData(
                    SSL_HDR_LEN + SSL_HANDSHAKE_HDR_LEN + SSL_HANDSHAKE_VERSION_LEN - msg_hdr_nw,
                ));
            }
            let r =
                self.fill_ssl_client_hello_handshake_header(left, &mut msg_hdr[msg_hdr_nw..])?;
            let Some(nw) = r else {
                return Ok(None);
            };
            msg_hdr_nw += nw;
            if msg_hdr_nw >= msg_hdr.len() {
                return self.check_ssl_client_hello_full_handshake_header(fragment_len, &msg_hdr);
            }
            offset += SSL_HDR_LEN + nw;
        }
    }

    fn check_ssl_client_hello_full_handshake_header(
        &mut self,
        fragment_len: usize,
        buf: &[u8],
    ) -> Result<Option<Protocol>, ProtocolInspectError> {
        /*
         * check ssh handshake hdr
         * struct ssl_handshake_hdr {
         *     __be32 hdr;
         * } __attribute_packed__;
         */
        if buf[0] != HandshakeType::ClientHello as u8 {
            self.exclude_current();
            return Ok(None);
        }
        let handshake_payload_len = u32::from_be_bytes([0u8, buf[1], buf[2], buf[3]]) as usize;
        if handshake_payload_len + SSL_HANDSHAKE_HDR_LEN < fragment_len {
            // it's possible that the handshake payload fragmented in multiple record, but
            // the first record must only contain the handshake message
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

    fn fill_ssl_client_hello_handshake_header(
        &mut self,
        data: &[u8],
        hdr_buf: &mut [u8],
    ) -> Result<Option<usize>, ProtocolInspectError> {
        let data_len = data.len();

        /*
         * check ssl hdr
         *
         * struct ssl_hdr {
         *     __u8   type;
         *     __be16 legacy_record_version;
         *     __be16 len;
         * } __attribute_packed__;
         */
        if data_len < SSL_HDR_LEN {
            return Err(ProtocolInspectError::NeedMoreData(SSL_HDR_LEN - data_len));
        }

        if data[0] != ContentType::Handshake as u8 {
            self.exclude_current();
            return Ok(None);
        }

        if check_legacy_version(data[1], data[2]).is_err() {
            self.exclude_current();
            return Ok(None);
        }
        let Ok(fragment_len) = check_fragment_len(data[3], data[4]) else {
            self.exclude_current();
            return Ok(None);
        };

        let copy_size = fragment_len.min(hdr_buf.len());
        if data_len < SSL_HDR_LEN + copy_size {
            return Err(ProtocolInspectError::NeedMoreData(
                SSL_HDR_LEN + copy_size - data_len,
            ));
        }

        let left = &data[SSL_HDR_LEN..];
        unsafe {
            std::ptr::copy_nonoverlapping(left.as_ptr(), hdr_buf.as_mut_ptr(), copy_size);
        }
        Ok(Some(copy_size))
    }
}

fn check_legacy_version(byte0: u8, byte1: u8) -> Result<Protocol, ()> {
    match (byte0, byte1) {
        (0x01, 0x01) => Ok(Protocol::TlsTlcp),
        (0x02, 0x00) | (0x03, 0x00) => Ok(Protocol::SslLegacy),
        (0x03, 0x01) | (0x03, 0x02) => Ok(Protocol::TlsLegacy),
        (0x03, 0x03) => Ok(Protocol::TlsModern),
        _ => Err(()),
    }
}

fn check_fragment_len(byte0: u8, byte1: u8) -> Result<usize, ()> {
    let ssl_payload_len = u16::from_be_bytes([byte0, byte1]) as usize;
    if ssl_payload_len == 0 || ssl_payload_len > 1 << 14 {
        // The length MUST NOT exceed 2^14 bytes.
        Err(())
    } else {
        Ok(ssl_payload_len)
    }
}
